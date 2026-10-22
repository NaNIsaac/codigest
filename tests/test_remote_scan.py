"""`cdg scan OWNER/REPO --github` against a fake GitHub on this machine (no network)."""

import json
import os
import tempfile
import time

import pytest
from fake_github import FakeGitHub
from typer.testing import CliRunner

from codigest.core import remote
from codigest.main import app

TOKEN = "fixture-token-not-a-secret"

SAMPLE = {
    ".gitignore": b"build/\n*.log\n!keep.log\n",
    "README.md": b"# sample\n",
    "src/app.py": b"from util.helpers import twice\n\n\ndef main():\n    return twice(2)\n",
    "src/util/helpers.py": b'def twice(x):\n    """Double."""\n    return 2 * x\n',
    "src/data.bin": b"\x00\xff\x10binary",
    "src/crlf.txt": b"one\r\ntwo\r\n",
    "docs/한글.md": "제목\n".encode(),
    "build/out.py": b"generated = True\n",
    "debug.log": b"noise\n",
    "keep.log": b"kept by negation\n",
    "node_modules/x/index.js": b"module.exports = 1\n",
    ".env": b"PASSWORD=fixture\n",
    "keys/id_rsa": b"not a key\n",
    # Built here so no token-shaped string sits in the repository.
    "config.py": b'GITHUB = "ghp_' + b"A" * 36 + b'"\n',
}


def write(root, files):
    for name, data in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


@pytest.fixture
def github(tmp_path, monkeypatch):
    repo = tmp_path / "sample"
    write(repo, SAMPLE)
    work = tmp_path / "work"
    work.mkdir()
    temp = tmp_path / "temp"
    temp.mkdir()
    monkeypatch.chdir(work)
    monkeypatch.setattr(tempfile, "tempdir", str(temp))
    for name in ("GH_TOKEN", "GITHUB_TOKEN"):
        monkeypatch.delenv(name, raising=False)
    fake = FakeGitHub(repo)
    with fake.running():
        monkeypatch.setenv("CODIGEST_GITHUB_API", fake.api)
        monkeypatch.setenv("CODIGEST_GITHUB_RAW", fake.raw)
        yield fake
    # Nothing left behind: no download folder, nothing in the working directory.
    assert list(temp.iterdir()) == []
    assert list(work.iterdir()) == []


@pytest.fixture
def sleeps(monkeypatch):
    waited = []
    monkeypatch.setattr(remote.time, "sleep", waited.append)
    return waited


def run(*args):
    return CliRunner().invoke(app, ["scan", *args])


def local(repo, monkeypatch, *args):
    """The same scan on the folder the fake serves, as a local project."""
    with monkeypatch.context() as m:
        m.chdir(repo)
        return run(*args)


def body(stdout: str) -> str:
    """Everything but the root element's opening line, which names the project."""
    return stdout.split("\n", 1)[1]


def test_the_whole_repository_comes_out_as_a_local_scan_of_it(github, monkeypatch):
    remote_result = run("octo/sample", "--github", "-y")
    local_result = local(github.root, monkeypatch, "-y")

    assert remote_result.exit_code == 0, remote_result.output
    assert body(remote_result.stdout) == body(local_result.stdout)
    assert remote_result.stdout.split("\n", 1)[0] == (
        '<project_snapshot project="octo/sample" source="https://github.com/octo/sample" '
        f'ref="main" commit="{github.commit}">'
    )
    assert "Withheld (may contain secrets): .env, keys/id_rsa" in remote_result.stderr
    assert "Withheld (may contain secrets): .env, keys/id_rsa" in local_result.stderr
    assert "ghp_" not in remote_result.stdout
    assert "<<Binary or Non-UTF8 Content>>" in remote_result.stdout


@pytest.mark.parametrize(
    "paths",
    [
        ["src/"],
        ["src/util", "keep.log"],
        ["src/app.py:4-5", "README.md"],
        ["build/out.py"],
        ["build", ".env", "src/app.py"],
        ["src", "src/util/helpers.py", "keep.log", "debug.log"],
        [".env"],
        ["docs/한글.md"],
    ],
)
def test_paths_select_what_local_targets_select(github, monkeypatch, paths):
    remote_result = run("octo/sample", "--github", "-y", *(a for p in paths for a in ("--path", p)))
    local_result = local(github.root, monkeypatch, "-y", *paths)

    assert remote_result.exit_code == 0, remote_result.output
    assert body(remote_result.stdout) == body(local_result.stdout)


def test_url_forms_name_the_ref_and_path(github):
    by_url = run("https://github.com/octo/sample/tree/main/src/util", "-y")
    by_option = run("octo/sample", "--github", "--ref", "main", "--path", "src/util", "-y")

    assert by_url.exit_code == 0, by_url.output
    assert by_url.stdout == by_option.stdout
    assert (
        by_url.stdout
        == run("https://github.com/octo/sample.git", "-y", "--path", "src/util").stdout
    )


def test_files_that_are_not_shared_are_never_downloaded(github):
    result = run("octo/sample", "--github", "-y", "--max-file-size", "12")

    assert result.exit_code == 0, result.output
    fetched = set(github.fetched())
    assert not fetched & {".env", "keys/id_rsa", "build/out.py", "debug.log"}
    assert not any(p.startswith("node_modules/") for p in fetched)
    assert "src/app.py" not in fetched  # over 12 bytes
    assert "Skipped (over 12 bytes): " in result.stderr
    assert "src/app.py" in result.stderr.split("Skipped (over 12 bytes): ")[1]
    assert "README.md" in fetched


def test_paths_outside_the_selection_are_never_downloaded(github):
    result = run("octo/sample", "--github", "-y", "--path", "src/util")

    assert result.exit_code == 0, result.output
    # The root .gitignore is read for its rules, as a local scan of src/util reads it.
    assert sorted(github.fetched()) == [".gitignore", "src/util/helpers.py"]


def test_the_repository_codigest_folder_is_neither_read_nor_applied(github):
    write(
        github.root,
        {
            ".codigest/prompts.toml": b'[prompts]\nsnapshot = "HIJACKED {project_name}"\n',
            ".codigest/config.toml": b'[filter]\nextensions = [".md"]\n',
        },
    )
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 0, result.output
    assert "HIJACKED" not in result.stdout
    assert '<file path="src/app.py">' in result.stdout
    assert not any(p.startswith(".codigest/") for p in github.fetched())


def test_only_get_requests_to_the_two_hosts(github):
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 0, result.output
    assert {r.method for r in github.requests} == {"GET"}
    assert {r.host for r in github.requests} == {"api", "raw"}


def test_no_token_is_sent_without_auth_even_when_one_is_set(github, monkeypatch):
    monkeypatch.setenv("GH_TOKEN", TOKEN)
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 0, result.output
    assert not any("authorization" in r.headers for r in github.requests)


def test_a_private_repository_needs_auth(github, monkeypatch):
    github.private_token = TOKEN
    monkeypatch.setenv("GH_TOKEN", TOKEN)

    refused = run("octo/sample", "--github", "-y")
    assert refused.exit_code == 1
    assert "octo/sample not found. If it is private, add --auth." in refused.stderr
    assert refused.stdout == ""

    github.requests.clear()
    allowed = run("octo/sample", "--github", "-y", "--auth")
    assert allowed.exit_code == 0, allowed.output
    assert '<file path="src/app.py">' in allowed.stdout
    # Files come from the API by content hash; the token goes nowhere else.
    assert {r.host for r in github.requests} == {"api"}
    assert all(r.headers.get("authorization") == f"Bearer {TOKEN}" for r in github.requests)
    assert TOKEN not in allowed.stdout + allowed.stderr


def test_a_token_that_cannot_read_the_repository(github, monkeypatch):
    github.private_token = TOKEN
    monkeypatch.setenv("GITHUB_TOKEN", "another-token")
    result = run("octo/sample", "--github", "-y", "--auth")

    assert result.exit_code == 1
    assert "octo/sample not found, or the token cannot read it." in result.stderr
    assert "another-token" not in result.output


def test_a_rejected_token_is_not_repeated_in_the_message(github, monkeypatch):
    monkeypatch.setenv("GH_TOKEN", TOKEN)
    github.queue["/repos/octo/sample"] = [(401, {}, b'{"message": "Bad credentials"}')]
    result = run("octo/sample", "--github", "-y", "--auth")

    assert result.exit_code == 1
    assert "GitHub rejected the token (401)." in result.stderr
    assert TOKEN not in result.output


def test_auth_without_any_token_says_where_one_comes_from(github, monkeypatch, tmp_path):
    monkeypatch.setenv("PATH", str(tmp_path / "no-bin"))
    result = run("octo/sample", "--github", "-y", "--auth")

    assert result.exit_code == 1
    assert "--auth needs GH_TOKEN, GITHUB_TOKEN or a logged-in gh." in result.stderr
    assert github.requests == []


def test_a_token_is_not_sent_in_clear_text_to_another_machine(monkeypatch):
    monkeypatch.setenv("CODIGEST_GITHUB_API", "http://example.com")
    with pytest.raises(remote.RemoteError, match="only over https"):
        remote.Client(TOKEN)


def test_a_moved_repository_is_followed_and_named(github):
    github.moved_from = "octo/old"
    result = run("octo/old", "--github", "-y", "--path", "README.md")

    assert result.exit_code == 0, result.output
    assert "octo/old moved to octo/sample" in result.stderr
    assert 'project="octo/sample"' in result.stdout


def test_a_redirect_to_another_host_is_refused(github):
    github.queue["/repos/octo/sample"] = [(302, {"Location": "http://192.0.2.1/x"}, b"")]
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 1
    assert "redirected to 192.0.2.1, which codigest does not contact" in result.stderr
    assert result.stdout == ""


def test_an_unknown_ref_names_the_default_branch(github):
    result = run("octo/sample", "--github", "--ref", "gone")

    assert result.exit_code == 1
    assert "No branch, tag or commit gone in octo/sample (default branch: main)." in " ".join(
        result.stderr.split()
    )


def test_a_renamed_default_branch_is_read_afresh(github):
    assert 'ref="main"' in run("octo/sample", "--github", "-y").stdout
    github.default_branch = "trunk"

    result = run("octo/sample", "--github", "-y")
    assert result.exit_code == 0, result.output
    assert 'ref="trunk"' in result.stdout


def test_a_ref_pins_every_download_to_one_commit(github):
    result = run("octo/sample", "--github", "--ref", "main", "-y")

    assert result.exit_code == 0, result.output
    raw = [r.path for r in github.requests if r.host == "raw"]
    assert raw and all(p.startswith(f"/octo/sample/{github.commit}/") for p in raw)


def test_the_rate_limit_is_reported_with_its_reset_time(github, sleeps):
    reset = int(time.time()) + 600
    github.queue["/repos/octo/sample"] = [
        (403, {"x-ratelimit-remaining": "0", "x-ratelimit-reset": str(reset)}, b"{}")
    ]
    result = run("octo/sample", "--github", "-y")

    when = time.strftime("%H:%M", time.localtime(reset))
    assert result.exit_code == 1
    assert f"GitHub API rate limit reached; resets at {when} (local time)." in result.stderr
    assert sleeps == []


def test_a_short_retry_after_is_waited_for(github, sleeps):
    github.queue["/repos/octo/sample"] = [(429, {"retry-after": "3"}, b"")]
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 0, result.output
    assert sleeps == [3]


def test_a_long_retry_after_is_reported(github, sleeps):
    github.queue["/repos/octo/sample"] = [(403, {"retry-after": "60"}, b"")]
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 1
    assert "GitHub asked to wait 60 s (secondary rate limit)." in result.stderr
    assert sleeps == []


def test_server_errors_are_retried_twice(github, sleeps):
    github.queue["/repos/octo/sample"] = [(502, {}, b""), (503, {}, b"")]
    assert run("octo/sample", "--github", "-y").exit_code == 0
    assert sleeps == [1, 2]

    github.queue["/repos/octo/sample"] = [(500, {}, b"")] * 3
    result = run("octo/sample", "--github", "-y")
    assert result.exit_code == 1
    assert "GitHub answered 500" in result.stderr


def test_a_failed_download_gives_no_partial_snapshot(github, sleeps):
    github.queue[f"/octo/sample/{github.commit}/README.md"] = [(500, {}, b"")] * 3
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 1
    assert result.stdout == ""


def test_an_unreachable_host_is_named(github, monkeypatch, sleeps):
    monkeypatch.setenv("CODIGEST_GITHUB_API", "http://127.0.0.1:9")
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 1
    assert "Could not reach 127.0.0.1:9" in result.stderr
    assert sleeps == [1, 2]


def test_a_truncated_listing_walks_down_to_the_named_paths(github):
    expected = run("octo/sample", "--github", "-y", "--path", "src/util").stdout
    github.truncate = True

    whole = run("octo/sample", "--github", "-y")
    assert whole.exit_code == 1
    assert "too large to list at once; name folders with --path" in whole.stderr

    scoped = run("octo/sample", "--github", "-y", "--path", "src/util")
    assert scoped.exit_code == 0, scoped.output
    assert scoped.stdout == expected


def test_entries_that_are_not_plain_files_are_skipped_and_named(github):
    odd = {"type": "blob", "mode": "100644", "size": 3, "sha": "0" * 40}
    github.extra = [
        {**odd, "path": "../evil.py"},
        {**odd, "path": "link.py", "mode": "120000"},
        {"path": "vendor", "type": "commit", "mode": "160000", "sha": "1" * 40},
    ]
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 0, result.output
    assert "Skipped (path not valid on this system): ../evil.py" in result.stderr
    assert "Skipped (symlink): link.py" in result.stderr
    assert "Skipped (submodule): vendor" in result.stderr
    assert not {"../evil.py", "link.py", "vendor"} & set(github.fetched())


def test_a_listed_path_cannot_be_written_outside_the_folder(tmp_path):
    with pytest.raises(remote.RemoteError, match="outside the temporary folder"):
        remote._write(tmp_path, "a/../../x.py", b"")


@pytest.mark.skipif(os.name != "nt", reason="names reserved on Windows")
def test_names_windows_cannot_hold_are_skipped():
    assert not remote.valid_here("src/aux.py")
    assert not remote.valid_here("a:b")
    assert not remote.valid_here("dot.")
    assert remote.valid_here("src/auxiliary.py")


def test_the_size_question_comes_before_any_download(github, monkeypatch):
    write(github.root, {f"many/{i:03}.py": b"x = 1\n" for i in range(101)})
    remote_result = run("octo/sample", "--github")
    local_result = local(github.root, monkeypatch)

    assert remote_result.exit_code == 1
    large = [line for line in remote_result.stderr.splitlines() if line.startswith("Large:")]
    assert large == [line for line in local_result.stderr.splitlines() if line.startswith("Large:")]
    assert "Stopped." in remote_result.stderr
    assert github.fetched() == [".gitignore"]


def test_a_hard_ceiling_applies_even_with_yes(github, monkeypatch):
    monkeypatch.setattr(remote, "MAX_FILES", 2)
    result = run("octo/sample", "--github", "-y")

    assert result.exit_code == 1
    assert "a GitHub scan takes at most 2 files" in result.stderr
    assert github.fetched() == [".gitignore"]


def test_markdown_and_json_name_the_source(github):
    markdown = run("octo/sample", "--github", "-y", "--format", "markdown", "--path", "README.md")
    assert markdown.stdout.startswith(
        f"# octo/sample\n\nSource: https://github.com/octo/sample, main at {github.commit}\n"
    )
    data = json.loads(run("octo/sample", "--github", "-y", "--format", "json").stdout)
    assert data["project"] == "octo/sample"
    assert data["source"] == {
        "url": "https://github.com/octo/sample",
        "ref": "main",
        "commit": github.commit,
    }


@pytest.mark.parametrize(
    ("raw", "owner", "repo", "ref", "paths"),
    [
        ("octo/sample", "octo", "sample", None, []),
        ("https://github.com/octo/sample", "octo", "sample", None, []),
        ("https://github.com/octo/sample/", "octo", "sample", None, []),
        ("https://github.com/octo/sample.git", "octo", "sample", None, []),
        ("http://github.com/octo/sample", "octo", "sample", None, []),
        ("github.com/octo/sample", "octo", "sample", None, []),
        ("https://github.com/octo/sample/tree/v1.0", "octo", "sample", "v1.0", []),
        ("https://github.com/octo/sample/tree/main/src/app", "octo", "sample", "main", ["src/app"]),
        ("https://github.com/octo/sample/blob/main/a%20b.py", "octo", "sample", "main", ["a b.py"]),
        ("https://github.com/octo/my.repo-1?tab=readme", "octo", "my.repo-1", None, []),
    ],
)
def test_accepted_targets(raw, owner, repo, ref, paths):
    assert remote.parse_target(raw) == remote.Target(owner, repo, ref, paths)


@pytest.mark.parametrize(
    "raw",
    [
        "octo",
        "octo/sample/extra",
        "-octo/sample",
        "octo/..",
        "https://github.com/octo",
        "https://github.com/octo/sample/issues/1",
        "https://github.com/octo/sample/tree/main/../../x",
    ],
)
def test_rejected_targets(raw):
    with pytest.raises(ValueError):
        remote.parse_target(raw)


@pytest.mark.parametrize(
    ("args", "message"),
    [
        (["--ref", "main"], "--ref and --path need a GitHub repository"),
        (["octo/sample", "src", "--github"], "name one GitHub repository and nothing else"),
        (["octo/sample", "--github", "-r"], "-r does not work on a GitHub repository yet"),
        (["octo/sample/extra", "--github"], "is not OWNER/REPO"),
        (["octo/sample", "--github", "--path", "../x"], "leaves the repository"),
        (["octo/sample", "--github", "--path", "/etc"], "not a path inside the repository"),
        (["octo/sample", "--github", "--max-file-size", "big"], "is not a size"),
    ],
)
def test_usage_errors_stop_before_any_request(github, args, message):
    result = run(*args)

    assert result.exit_code == 2
    assert message in " ".join(result.stderr.replace("│", " ").split())
    assert github.requests == []


def test_a_missing_local_path_that_looks_like_a_repository_suggests_github(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    result = run("octo/sample")

    assert result.exit_code == 2
    assert "add --github to scan the GitHub repository octo/sample" in " ".join(
        result.stderr.replace("│", " ").split()
    )


@pytest.mark.parametrize(
    ("raw", "size"),
    [("1MB", 1_000_000), ("500KB", 500_000), ("20000", 20_000), ("2 gb", 2 * 10**9)],
)
def test_sizes(raw, size):
    assert remote.parse_size(raw) == size
