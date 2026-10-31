# Remote scan: `cdg scan` on a GitHub repository

Status: implemented in the Python CLI and opt-in MCP `scan_github`; the Rust build is not. Written against `c1c4d11` (0.12.2) as the plan for the pull requests that follow it, and
corrected where the implementation turned out different.

## Goal

Share a GitHub repository with a model the way `cdg scan` shares a local project, without
cloning it first:

```bash
cdg scan NaNIsaac/QoraGeom --github
cdg scan https://github.com/NaNIsaac/QoraGeom
cdg scan NaNIsaac/QoraGeom --github --ref main
cdg scan NaNIsaac/QoraGeom --github --path src/
```

The output is the same `<project_snapshot>` (or Markdown, or JSON) that a local scan of a
checkout of that commit would give, with the same files left out, the same secrets masked and
the same size question.

Out of scope for the first version: hosts other than github.com (GitHub Enterprise, GitLab),
`-r` on a remote repository, and remote `digest`, `tree`, `diff`, `pack`. Section
[Later](#later) says which of these the design makes cheap.

### Where the idea comes from

`project_to_markdown` (one file, `main.py`, 126 lines) walks a local folder and writes a
Markdown file with a tree, a table of files with line counts, and each file with a supported
extension cut at 500 lines. It makes no network requests and has no notion of a remote
repository; the remote part is new here. Its features against codigest's:

| `project_to_markdown` | codigest today |
| --- | --- |
| directory tree, `EXCLUDE_DIRS` | `<project_structure>`, `ALWAYS_IGNORE` plus `.gitignore` |
| extension allow-list (`.py .yaml .yml .toml .json .md .txt`) | `[filter] extensions` in `.codigest/config.toml` (off by default) |
| Markdown output | `--format markdown` |
| per-file line-count table | none; `scan` reports file count and estimated tokens |
| `--max-lines` cut per file | none; `FILE:START-END` ranges, and the size question before copying |
| no secret handling | credential files withheld, tokens masked |

Nothing in the left column is missing from codigest except the line-count table and the
per-file cut, and neither is needed for remote scan. Whether `project_to_markdown` can be
archived is for Isaac to decide after remote scan is released and checked; this work does not
touch that repository.

### Fit with #1 and #24

`scan` is the extraction core that #1 puts first ("let developers choose what code context to
share"). Remote scan widens what can be chosen, not what is chosen automatically, so it fits
that direction. #24 points out that every behavior change is made twice (Python and Rust) and
that new surface should wait for privacy fixes in capture. Remote scan does not touch capture;
it is built in both builds with a parity test, as every other command is (Python is the
reference, `rust/README.md`).

## User interface

### Input forms

| Form | Example | Meaning |
| --- | --- | --- |
| `OWNER/REPO --github` | `cdg scan NaNIsaac/QoraGeom --github` | default branch, whole repository |
| URL | `cdg scan https://github.com/NaNIsaac/QoraGeom` | same; `--github` is implied |
| URL with ref and path | `cdg scan https://github.com/NaNIsaac/QoraGeom/tree/main/src` | ref `main`, path `src` |
| `--ref REF` | `cdg scan NaNIsaac/QoraGeom --github --ref v0.3.0` | branch, tag, or full or short commit SHA |
| `--path P` (repeatable) | `--path src/ --path README.md --path src/geom.py:40-80` | only these paths, line ranges as for local targets |

Parsing rules:

- A URL is recognized only by the prefix `https://github.com/` (also `http://`, rewritten to
  `https`, and `github.com/` without a scheme). A trailing `.git` and a trailing `/` are
  dropped. `/tree/<ref>/<path>` and `/blob/<ref>/<path>` set ref and path. The first segment
  after `tree`/`blob` is taken as the ref; a branch name containing `/` needs `--ref` and
  `--path` instead (the URL alone is ambiguous, and resolving it would cost API calls).
- `OWNER/REPO` without `--github` stays a local path, exactly as today. If it does not exist,
  the existing "does not exist" error gains the hint "add --github to scan the GitHub
  repository OWNER/REPO". Nothing that works today changes meaning.
- `OWNER` and `REPO` must match GitHub's name rules (`[A-Za-z0-9-]` for owners,
  `[A-Za-z0-9._-]` for repositories, not `.` or `..`); anything else is a usage error (exit 2)
  before any request.
- One repository per scan. Mixing a repository with local targets, two repositories, or
  `--ref`/`--path` with `--github` absent and no URL is a usage error (exit 2).
- `--ref` given together with a URL that carries a ref: the option wins, and the two must agree
  on the repository.
- `--path` values are POSIX paths relative to the repository root. `..`, absolute paths and
  backslashes are usage errors.

### Options

New options on `scan` only:

| Option | Default | |
| --- | --- | --- |
| `--github` | off | treat the first target as `OWNER/REPO` on github.com |
| `--ref REF` | default branch | branch, tag or commit |
| `--path P` | whole repository | repeatable; `FILE:START-END` allowed |
| `--auth` | off | send a GitHub token (see [Authentication](#authentication)) |
| `--max-file-size SIZE` | `1MB` | larger files are listed as skipped and never downloaded |

Unchanged and applying to remote scans the same way: `-s`, `--copy`/`--no-copy`, `-o`, `-m`,
`-l`, `-y`, `--format`, `--preset`. `-a` has nothing to ignore (see [Filters](#filters-and-gitignore))
and is accepted. `-r` with a remote target is a usage error in the first version.

`--preset` lets a user save `--auth` and the other options under `[presets.scan.NAME]` in the
user config, which is the durable form of the explicit choice described below.

### Output

The body is what a local scan of a checkout of the same commit gives. The differences:

- The project name is `OWNER/REPO` instead of the folder name.
- The root element gets `source`, `ref` and `commit` attributes, only for remote scans:
  `<project_snapshot project="NaNIsaac/QoraGeom" source="https://github.com/NaNIsaac/QoraGeom" ref="main" commit="<40 hex>">`.
  Markdown gets a `Source:` line under the heading, JSON a `source` object. A local scan's
  output stays byte for byte as it is (existing tests and parity tests keep passing), and
  `prompts.toml` templates receive the three values as extra variables they may ignore.
- The summary on stderr (printed, as for local scans, only when the result is not printed)
  names the repository and the commit:
  `Snapshot NaNIsaac/QoraGeom@main (1a2b3c4) 12 files, ~3,000 tokens`.
- Skipped files are listed on stderr like withheld ones, by reason: `Withheld (may contain
  secrets)`, `Skipped (over 1 MB)`, `Skipped (symlink)`, `Skipped (submodule)`,
  `Skipped (path not valid on this system)`.

## Pipeline

### Where it joins the local pipeline

The local pipeline (`site/architecture.md`, "Data flow") is: find the root, select files
(`scanner`, then `is_secret_file`), read and wrap them, then `OutputHandler`, which masks
tokens and copies or prints.

Remote scan adds one stage in front of it and reuses the rest unchanged:

```text
parse target ──> resolve ref to commit ──> list tree ──> select ──> size check ──> download selected
                     (API)                   (API)     (same rules)  (same numbers)     (raw / API)
                                                                                           │
                                                          write them into a temporary folder
                                                                                           │
                       local scan of that folder, root fixed, project name OWNER/REPO <────┘
                       (scanner, secret-file check, processor, tags, prompts, OutputHandler)
                                                                                           │
                                                                     delete the temporary folder
```

1. **Resolve.** `GET /repos/{owner}/{repo}` gives `default_branch` and tells "not found or
   private" apart from other failures. `GET /repos/{owner}/{repo}/commits/{ref}` with
   `Accept: application/vnd.github.sha` turns the ref (or the default branch) into one commit
   SHA. Every later request names that SHA, so a branch that moves, or a default branch renamed
   during the scan, cannot mix two states.
2. **List.** `GET /repos/{owner}/{repo}/git/trees/{sha}?recursive=1` gives every path with its
   type (`blob`, `tree`, `commit` for submodules), mode (`120000` for symlinks) and size,
   without downloading content.
3. **Select.** The listing is filtered by the same rules the local scanner applies while walking
   (next section), then by `is_secret_file`, then by `--max-file-size`. The root `.gitignore`
   is the only content fetched before selection.
4. **Size check.** The estimate is the sum of the selected sizes over four, as the local scan
   computes it from `stat`, with the same thresholds (30,000 tokens or 100 files), the same
   question, the same stop when stdin is not a terminal, and the same `-y`. It runs before any
   file is downloaded. One difference: a file named with a line range counts with its whole
   size, because its lines are not known before it is downloaded (a local scan counts only the
   range).
5. **Download** only the selected blobs (see [Requests](#requests)).
6. **Materialize** them into a new temporary folder (`tempfile` / `std::env::temp_dir` with a
   random name, created with owner-only permissions on Unix) at their repository paths.
7. **Scan** that folder with the existing code: `ProjectContext` gets the folder as a fixed
   root (a new optional argument; today the root is found by walking up from the current
   directory, which must not happen here) and `OWNER/REPO` as the project name; `--path`
   values become targets inside it. The size check is not asked a second time. A named path
   with nothing selected under it (ignored, or a withheld secret file) was not downloaded, and
   shares nothing, as a local scan of it does; it is not replaced by the whole folder. A line
   range on a `--path` that is not a file of the repository is an error.
8. **Clean up.** The folder is removed in a `finally` / `Drop`, also on error and Ctrl+C.

Why a temporary folder rather than teaching every stage to read from a listing: the scanner,
the tree printer, the line-range reader, the block cache and the three output formats all take
paths on disk, in both builds. Feeding them real files keeps one code path, so a remote scan
cannot drift from a local one, and it makes remote `digest` and `tree` almost free later. The
cost is writing the selected files once to a temporary folder, which is bounded by the same
size check.

The block cache (`.codigest/cache.json`) is only saved where `.codigest/` already exists, and
remote `.codigest/` is never downloaded (next section), so nothing is cached between remote
scans.

### Filters and `.gitignore`

Selection on the listing must give exactly the set the local scanner gives on a checkout. The
local scanner walks from the root, drops an ignored directory without entering it, and keeps a
file that is not ignored and passes the extension filter. For a flat listing that is: a file
is kept when none of its ancestor directories (`a/`, `a/b/`, matched with a trailing `/`) is
ignored and the file itself is not ignored. That is the same order of decisions, so negation
patterns behave the same (a file under an ignored directory cannot be re-included, in both).
The matcher is the existing one (`pathspec` in Python, `gitwild.rs` in Rust), fed the same
pattern list: `ALWAYS_IGNORE`, then the repository's root `.gitignore`.

Like the local scanner, only the root `.gitignore` is read; nested ones are not, in either
case. Changing that is a separate change to both scanners.

The repository's `.codigest/` folder is never downloaded (it is in `ALWAYS_IGNORE`, as locally),
so neither its `config.toml` filters nor its `prompts.toml` templates apply to a remote scan.
Templates are format strings that decide what the output looks like; a repository someone else
controls should not decide that. Filters for a remote scan come from `--path` and from a
`--preset`.

Tracked files that match `.gitignore` are left out, as they are when scanning a local checkout.

### Entries that are not plain files

| Entry | Local scan of a checkout | Remote scan |
| --- | --- | --- |
| symlink (`120000`) | followed if it points at a file | skipped and listed; the target may be outside the repository |
| submodule (`commit`) | an empty folder, nothing shown | skipped and listed |
| path invalid on this OS (`aux.py`, `a:b`, trailing dot on Windows) | the checkout fails | skipped and listed |
| two paths differing only in case on a case-insensitive file system | one overwrites the other | the second is skipped and listed |
| binary or non-UTF-8 file | `<<Binary or Non-UTF8 Content>>` | the same (decided after download, by the same reader) |
| Git LFS pointer | the pointer text | the pointer text |

Symlinks are the one place the outputs differ on purpose. The parity fixture has no symlinks.

Every materialized path is joined under the temporary root and checked to stay inside it after
normalization, independent of the name checks above. A listing from a compromised or fake
server cannot write outside that folder.

## Requests

### Hosts and methods

| Purpose | Request | Counts against the REST API limit |
| --- | --- | --- |
| repository, default branch | `GET https://api.github.com/repos/{o}/{r}` | yes |
| ref to commit | `GET https://api.github.com/repos/{o}/{r}/commits/{ref}` | yes |
| listing | `GET https://api.github.com/repos/{o}/{r}/git/trees/{sha}?recursive=1` | yes |
| file, no token | `GET https://raw.githubusercontent.com/{o}/{r}/{sha}/{path}` | no (rate limited separately) |
| file, with `--auth` | `GET https://api.github.com/repos/{o}/{r}/git/blobs/{blob_sha}`, `Accept: application/vnd.github.raw+json` | yes |

A scan of a public repository without a token costs three REST calls plus one raw download per
selected file. The unauthenticated REST limit is 60 per hour per IP address, so about 20 scans
an hour. With `--auth`, files come from the blob endpoint so that the token is only ever sent
to `api.github.com`, and blobs are addressed by their content hash from the listing.

Every request is `GET`. The HTTP layer of both builds has one function, `get(url, headers)`,
and no way to send another method or a body. Requests carry `User-Agent: codigest/<version>`,
`X-GitHub-Api-Version: 2022-11-28` on API calls, a 30 s socket timeout, and go
through the proxy named in `HTTPS_PROXY` when set.

Redirects are followed for `GET` only, at most five, and the `Authorization` header is dropped
when a redirect leaves the host it was sent to (Python: `add_unredirected_header` plus a
redirect handler that checks the host; Rust: `ureq` configured never to forward it, checked by
a test rather than assumed from its defaults). A renamed
or transferred repository answers the API with `301` to the new location; the scan follows it
and says `NaNIsaac/old moved to NaNIsaac/new` on stderr.

Downloads run four at a time. Results are sorted by path before materializing, so the output
does not depend on arrival order.

### Failures, rate limits, retries

| Response | Action | Message (stderr, exit 1) |
| --- | --- | --- |
| `404` on the repository, no `--auth` | stop | `OWNER/REPO not found. If it is private, add --auth.` |
| `404` on the repository with `--auth` | stop | `OWNER/REPO not found, or the token cannot read it.` |
| `404`/`422` on the ref | stop | `No branch, tag or commit REF in OWNER/REPO (default branch: main).` |
| `401` | stop | `GitHub rejected the token (401).` |
| `403` or `429` with `x-ratelimit-remaining: 0` | stop | `GitHub API rate limit reached; resets at 14:05 (local time). --auth raises it.` |
| `403` or `429` with `retry-after` ≤ 10 s | wait, retry | (none unless it fails again) |
| `403` or `429` with `retry-after` > 10 s | stop | `GitHub asked to wait N s (secondary rate limit). Try again later.` |
| other `403` | stop | `GitHub refused the request (403): <message field>` |
| `5xx`, connection reset, timeout | retry twice, 1 s then 2 s | `Could not reach GitHub: <reason>` |
| DNS failure, no network | stop | `Could not reach api.github.com: <reason>` |
| listing with `truncated: true` | see below | |

Only the `message` field of a GitHub error body is shown, never headers or request details. A
download that fails after its retries fails the whole scan; a partial snapshot that looks
complete would be worse than none.

### Large repositories

- **Listing truncated** (the recursive listing stops at about 100,000 entries or 7 MB). With
  `--path`, the scan lists only what is needed: it walks from the root tree one level at a
  time (`GET .../git/trees/{tree_sha}`, non-recursive) down to each named folder, then lists
  that folder recursively. Without `--path`, it stops: `OWNER/REPO is too large to list at
  once; name folders with --path.`
- **Too much selected.** The size question (30,000 tokens or 100 files) is asked before
  downloading. With `-y` (and so through MCP) a hard ceiling still applies to remote scans:
  5,000 files or 50 MB of selected content, beyond which the scan stops and asks for `--path`.
  These two numbers are constants in the first version.
- **Large files.** `--max-file-size` (1 MB) skips them before download. The local scan has no
  such limit; on a checkout a 50 MB file would be read and shown as binary or as text. On a
  remote scan downloading it would be the expensive part, so it is skipped and listed instead.

## Authentication

- Without `--auth` no token is sent, even when `GH_TOKEN` or `GITHUB_TOKEN` is set in the
  environment. Private repositories then answer `404`, and the message suggests `--auth`.
- With `--auth` the token is, in this order: `GH_TOKEN`, `GITHUB_TOKEN`, then the output of
  `gh auth token` if `gh` is installed. None found: stop with
  `--auth needs GH_TOKEN, GITHUB_TOKEN or a logged-in gh.`
- The token is sent only to `api.github.com` (or the API base set for tests, see below, and
  then only when it is `https` or a loopback address), never to `raw.githubusercontent.com` and
  never across a redirect to another host.
- The token is never written to disk, logged, or put in an error message. Output passes through
  `redact_code`, which masks GitHub token shapes anyway.
- Recommended token: a fine-grained personal access token with read-only "Contents" on the
  repositories to be scanned. `gh auth token` gives a broader token; codigest still only sends
  `GET` with it.

## Read-only guarantee

1. The HTTP layer can only send `GET` (one function, no method parameter).
2. Requests go only to `api.github.com` and `raw.githubusercontent.com`; any other host,
   including after a redirect, is refused. The only exception is the test base URL below.
3. No `git` command runs against the remote, and `gh` is only asked for `auth token`.
4. Nothing is written to the current directory or the project it belongs to. The only writes
   are the temporary folder (removed at the end) and the user's own `-o FILE`.
5. No state is kept between runs: no cache, no stored token, no stored ETags.
6. Tests check 1, 2 and 4 against a fake server that records every request.

## MCP

The local `scan` tool and the default `tools/list` remain unchanged. In the Python build,
`cdg mcp --allow-remote` adds a separate `scan_github` tool taking `repository`, optional
`ref`, `paths` and `format`. An unenabled call is refused. Authentication is controlled
only by the operator's additional `--auth` startup flag, not tool arguments. GitHub content
is untrusted input, not agent instructions. The CLI's download bounds and secret masking
apply unchanged, and each remote CLI process has a 110-second timeout.

This source change is not in the Rust build. Tests cover tool discovery, denied remote
access, unchanged local path boundaries, argument validation, credential opt-in, and an
actual stdio-to-CLI-to-fake-GitHub scan. See `site/chatgpt-mcp.md` for private connectivity.

## Tests

All tests run without network access unless marked otherwise.

### Fake GitHub

The tests write a small repository into a temporary folder (from a table in the test, so that no
token-shaped string is committed). A fake server (Python: `tests/fake_github.py`, `http.server`
on two ports of `127.0.0.1` for the API and raw hosts; Rust: a `std::net::TcpListener` thread in
`rust/tests/common`) builds the API answers and the raw files from such a folder, and
records method, host, path and headers of every request. Both builds take the base URLs from
`CODIGEST_GITHUB_API` and `CODIGEST_GITHUB_RAW`; these are for tests and are documented as such.
Scenarios are switched per test: rate limited, secondary limit with `Retry-After`, a `500`
before success, `404` for a private repository, a moved repository (`301`), a truncated listing,
a renamed default branch between two runs.

### Cases

- **Same output as local.** For each fixture repository, the remote scan's output equals the
  local scan of the same folder except for the root element's attributes. This is the test that
  keeps "the same pipeline" true. Fixtures cover a `.gitignore` with negation and directory
  patterns, `ALWAYS_IGNORE` folders, a `.env` and an `id_rsa`, a file with a GitHub-shaped
  token in it, a binary file, CRLF line endings, non-ASCII paths, and `--path` with folders,
  files and line ranges.
- **Never fetched.** Withheld secret files, ignored files, `.codigest/`, files over
  `--max-file-size` and everything outside `--path` do not appear in the request log.
- **Read-only.** Every recorded request is `GET` to an allowed host; the working directory has
  the same entries before and after; the temporary folder is gone after success, after a failed
  download and after an error in rendering.
- **Authentication.** No `Authorization` header without `--auth`, even with `GH_TOKEN` set;
  with `--auth`, the header is present on API requests only and absent after a cross-host
  redirect; the token does not appear in stdout, stderr or `-o` output on any failure path.
- **Input forms.** A table of accepted and rejected targets (`OWNER/REPO`, URLs with `.git`,
  trailing `/`, `/tree/ref/path`, `/blob/ref/file`, bad names, `..`, two repositories, mixed
  local and remote) with the parsed owner, repository, ref and paths or the error and exit code.
- **Failures.** Each row of the failure table: message, exit code, number of retries, and no
  partial output.
- **Hostile listing.** Entries with `../`, absolute paths, Windows reserved names and case
  collisions are skipped and listed, and nothing is written outside the temporary folder.
- **Size.** The question is asked with the numbers a local scan of the same files would show,
  before any download; non-interactive runs stop; the hard ceiling applies with `-y`.
- **MCP.** `tools/list` is byte-identical without `--allow-remote`; a remote call is refused
  without it and served with it.
- **Parity.** The Rust integration tests run the same scenarios against their fake server, and
  under `CODIGEST_PARITY=1` compare the Rust and Python output byte for byte, as for the other
  commands.

### With network (opt-in)

One test per build, skipped unless `CODIGEST_NETWORK=1`, scans a tag of this repository on
github.com with `--path` and compares it with a local scan of the same tag. It is not part of
CI by default; it is run by hand before the release that includes remote scan, and its result
goes into that pull request.

## Dependencies

- Python: `urllib.request`, `http.client`, `concurrent.futures`, `tempfile` from the standard
  library. No new dependency.
- Rust: `ureq` 3 with `rustls` (and its default bundled root certificates), as the one new
  dependency: there is no HTTP client in the tree today, and a blocking client fits the rest of
  the program, which has no async runtime. Its effect on the release binary size (4.6 MB today)
  and on the Android and musl targets is measured in the implementation pull request.

Alternatives considered:

- **`git clone --depth 1 --filter=blob:none` and a sparse checkout.** Needs `git` on `PATH`,
  uses whatever credential helper is configured (so a private repository could be read without
  an explicit choice), writes a repository to disk, and its failures are Git's messages rather
  than GitHub's rate-limit information. Rejected.
- **Downloading the tarball.** One request, but it brings every file, including the ones the
  user did not select and the secret files that are withheld, onto the machine, and it is the
  size of the whole repository. Rejected as the default; if raw downloads turn out to be rate
  limited in practice, it can be added for large selections.
- **Calling `curl` or `gh api`.** Avoids a Rust dependency, but adds a run-time dependency that
  is missing on some targets (Termux, minimal containers), and passing a token on a command line
  exposes it to other processes. Rejected.
- **GraphQL.** Needs a token for every request. Rejected.

## Pull requests

1. This document.
2. Python: target parsing, HTTP layer, resolve/list/select/download/materialize, the fixed-root
   argument of `ProjectContext`, the `scan` options and help text, the fake server and the tests
   above. `docs/remote-scan.md` updated where the implementation differs.
3. Rust: the same, with `ureq`, the Rust fake server and the parity tests.
4. MCP `--allow-remote` in both builds, with its tests.

The README (both languages) and the documentation site are not changed by these pull requests.
After 2–4 are merged and the opt-in network test has passed, one issue lists the README and
site changes, including the line in "Privacy and safety" that says Codigest makes no network
requests, which stops being true for `--github` and must say exactly when it is not.

## Later

Not in this work, and what the design leaves for them:

- `digest` and `tree` on a remote repository: the same materialized folder, a different command
  on it.
- `-r` on a remote repository: needs the files that imports point at, which are only known
  after the first files are read; a second round of downloads restricted to the listing.
- Nested `.gitignore` files: a change to both local scanners first, then the remote selection
  follows by construction.
- GitHub Enterprise and other hosts: the base URLs are already configurable for tests; supporting
  them needs host-specific raw URLs and a decision on where a token may be sent.
