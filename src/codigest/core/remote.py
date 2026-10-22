"""
A GitHub repository read through the same pipeline as a local project (docs/remote-scan.md).

The ref is pinned to one commit, the listing is filtered with the local scanner's rules, and only
the selected files are downloaded, into a temporary folder that the local scan then reads.
Requests are GET only, to the GitHub API and raw hosts; a token is sent only when asked for.
"""

import contextlib
import os
import re
import sys
from collections.abc import Generator
from dataclasses import dataclass, field
from pathlib import Path
lazy import json
lazy import subprocess
lazy import tempfile
lazy import time
lazy import urllib.error
lazy import urllib.parse
lazy import urllib.request
lazy from concurrent.futures import ThreadPoolExecutor
lazy from importlib.metadata import PackageNotFoundError, version

lazy from rich.filesize import decimal

from . import scanner
from .common import remove_tree
from .redact import is_secret_file

API = "https://api.github.com"
RAW = "https://raw.githubusercontent.com"
API_VERSION = "2022-11-28"
TIMEOUT = 30
RETRIES = 2
BACKOFF = (1, 2)
# A Retry-After longer than this is reported instead of waited for.
MAX_WAIT = 10
DOWNLOADS = 4
# Bounds a remote scan even with -y (and so through MCP), where the size question is skipped.
MAX_FILES = 5_000
MAX_BYTES = 50_000_000

_OWNER = re.compile(r"[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?")
_REPO = re.compile(r"[A-Za-z0-9._-]+")
_URL = re.compile(r"^(?:https?://)?(?:www\.)?github\.com/(?P<rest>.*)$", re.IGNORECASE)
_SHA = re.compile(r"[0-9a-f]{40}")
# As for local targets (commands/scan.py): FILE:40-60 or FILE:42.
_RANGE = re.compile(r"^(?P<path>.+):(?P<lo>\d+)(?:-(?P<hi>\d+))?$")
_SIZE = re.compile(r"^(\d+)\s*(B|KB|MB|GB)?$", re.IGNORECASE)
_UNITS = {"": 1, "B": 1, "KB": 1_000, "MB": 1_000_000, "GB": 1_000_000_000}
_RESERVED = re.compile(r"^(con|prn|aux|nul|com[0-9¹²³]|lpt[0-9¹²³])(\..*)?$", re.IGNORECASE)
_LOOPBACK = {"localhost", "127.0.0.1", "::1"}


class RemoteError(Exception):
    """A failure to report as it is (exit 1)."""


class HttpError(RemoteError):
    def __init__(self, status: int, message: str):
        super().__init__(message)
        self.status = status


@dataclass(slots=True)
class Target:
    owner: str
    repo: str
    ref: str | None = None
    paths: list[str] = field(default_factory=list)

    @property
    def name(self) -> str:
        return f"{self.owner}/{self.repo}"


@dataclass(slots=True, frozen=True)
class Entry:
    path: str
    kind: str  # blob, tree, or commit (a submodule)
    mode: str
    size: int
    sha: str


@dataclass(slots=True)
class Plan:
    """What a remote scan will download, decided before any file is fetched."""

    client: Client
    name: str
    ref: str
    commit: str
    # The paths named with --path and their line ranges (":40-60" or "").
    targets: list[tuple[str, str]]
    files: list[Entry]
    withheld: list[str]
    skipped: dict[str, list[str]]
    notes: list[str]
    gitignore: bytes | None

    @property
    def source(self) -> str:
        return f"https://github.com/{self.name}"

    @property
    def size(self) -> int:
        return sum(e.size for e in self.files)


def is_url(raw: str) -> bool:
    return bool(_URL.match(raw))


def parse_target(raw: str) -> Target:
    """OWNER/REPO, or a github.com URL with an optional /tree/REF/PATH or /blob/REF/PATH."""
    ref, paths = None, []
    if m := _URL.match(raw):
        rest = m["rest"].split("#", 1)[0].split("?", 1)[0].strip("/")
        parts = [urllib.parse.unquote(p) for p in rest.split("/")]
        if len(parts) < 2 or not all(parts[:2]):
            raise ValueError(f"{raw} does not name a repository (https://github.com/OWNER/REPO)")
        owner, repo, *tail = parts
        repo = repo.removesuffix(".git")
        if tail:
            if tail[0] not in ("tree", "blob") or len(tail) < 2:
                raise ValueError(f"{raw} is not a link to a repository, folder or file")
            ref = tail[1]
            if len(tail) > 2:
                paths = [repo_path("/".join(tail[2:]))]
    else:
        parts = raw.split("/")
        if len(parts) != 2:
            raise ValueError(f"{raw} is not OWNER/REPO")
        owner, repo = parts
    if not _OWNER.fullmatch(owner) or not _REPO.fullmatch(repo) or repo in (".", ".."):
        raise ValueError(f"{owner}/{repo} is not a GitHub repository name")
    return Target(owner, repo, ref, paths)


def repo_path(raw: str) -> str:
    """A path inside the repository as POSIX relative to its root ("" is the root)."""
    if "\\" in raw or raw.startswith("/") or re.match(r"^[A-Za-z]:", raw):
        raise ValueError(f"{raw} is not a path inside the repository (use a/b, relative)")
    parts = [p for p in raw.split("/") if p not in ("", ".")]
    if ".." in parts:
        raise ValueError(f"{raw} leaves the repository")
    return "/".join(parts)


def _split_range(raw: str) -> tuple[str, str]:
    """`src/a.py:40-60` -> ("src/a.py", ":40-60"); a plain path has an empty range."""
    if m := _RANGE.match(raw):
        return m["path"], raw[m.end("path") :]
    return raw, ""


def repo_target(raw: str) -> str:
    """A --path value, checked and normalized: a path inside the repository, optionally with
    a line range as for local targets."""
    path, span = _split_range(raw)
    return repo_path(path) + span


def parse_size(raw: str) -> int:
    if not (m := _SIZE.match(raw.strip())):
        raise ValueError(f"{raw} is not a size (1MB, 500KB, 20000)")
    return int(m[1]) * _UNITS[(m[2] or "").upper()]


def token() -> str:
    """The token --auth sends: GH_TOKEN, GITHUB_TOKEN, then `gh auth token`."""
    for name in ("GH_TOKEN", "GITHUB_TOKEN"):
        if value := os.environ.get(name, "").strip():
            return value
    try:
        result = subprocess.run(
            ["gh", "auth", "token"], capture_output=True, text=True, timeout=10, check=False
        )
    except OSError, subprocess.TimeoutExpired:
        result = None
    if result and result.returncode == 0 and (value := result.stdout.strip()):
        return value
    raise RemoteError("--auth needs GH_TOKEN, GITHUB_TOKEN or a logged-in gh.")


def _netloc(url: str) -> str:
    return urllib.parse.urlsplit(url).netloc.lower()


def _user_agent() -> str:
    try:
        return f"codigest/{version('codigest')}"
    except PackageNotFoundError:
        return "codigest/dev"


def _redirects(client: Client) -> urllib.request.HTTPRedirectHandler:
    """Follows redirects only between the hosts codigest talks to, and gives the token back
    only to the API host (urllib drops unredirected headers on every redirect). Defined here so
    that importing this module does not load urllib."""

    class Redirects(urllib.request.HTTPRedirectHandler):
        max_redirections = 5

        def redirect_request(self, req, fp, code, msg, headers, newurl):
            if (host := _netloc(newurl)) not in client.hosts:
                raise RemoteError(f"GitHub redirected to {host}, which codigest does not contact.")
            new = super().redirect_request(req, fp, code, msg, headers, newurl)
            if new is not None:
                client.authorize(new)
            return new

    return Redirects()


class Client:
    """GET requests to the GitHub API and raw hosts, nothing else."""

    def __init__(self, token: str | None = None):
        # Overridable for tests (a fake server on this machine).
        self.api = os.environ.get("CODIGEST_GITHUB_API", API).rstrip("/")
        self.raw = os.environ.get("CODIGEST_GITHUB_RAW", RAW).rstrip("/")
        self.hosts = {_netloc(self.api), _netloc(self.raw)}
        api = urllib.parse.urlsplit(self.api)
        if token and api.scheme != "https" and api.hostname not in _LOOPBACK:
            raise RemoteError(f"--auth sends the token only over https, not to {self.api}.")
        self.token = token
        self.user_agent = _user_agent()
        self._opener = urllib.request.build_opener(_redirects(self))

    def authorize(self, request: urllib.request.Request) -> None:
        if self.token and _netloc(request.full_url) == _netloc(self.api):
            request.add_unredirected_header("Authorization", f"Bearer {self.token}")

    def get(self, url: str, accept: str = "application/vnd.github+json") -> tuple[bytes, str]:
        """Body and final URL of a GET; HttpError for an answer other than 2xx."""
        request = urllib.request.Request(
            url, headers={"Accept": accept, "User-Agent": self.user_agent}, method="GET"
        )
        if _netloc(url) == _netloc(self.api):
            request.add_header("X-GitHub-Api-Version", API_VERSION)
        self.authorize(request)
        for attempt in range(RETRIES + 1):
            try:
                with self._opener.open(request, timeout=TIMEOUT) as response:
                    return response.read(), response.url
            except urllib.error.HTTPError as e:
                with e:
                    status, headers, body = e.code, e.headers, e.read()
                wait = _retry_wait(status, headers, attempt)
                if wait is None:
                    raise _http_error(status, headers, body) from None
            except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
                if attempt == RETRIES:
                    reason = getattr(e, "reason", None) or e
                    raise RemoteError(f"Could not reach {_netloc(url)}: {reason}") from None
                wait = BACKOFF[attempt]
            time.sleep(wait)
        raise AssertionError("unreachable")

    def json(self, path: str) -> dict:
        body, _ = self.get(f"{self.api}{path}")
        return json.loads(body)


def _retry_wait(status: int, headers, attempt: int) -> float | None:
    """Seconds to wait before trying again, or None to give up."""
    if attempt == RETRIES:
        return None
    if status in (403, 429):
        if headers.get("x-ratelimit-remaining") == "0":
            return None
        if (after := headers.get("retry-after", "")).isdigit() and int(after) <= MAX_WAIT:
            return int(after)
        return None
    return BACKOFF[attempt] if status >= 500 else None


def _http_error(status: int, headers, body: bytes) -> HttpError:
    try:
        message = str(json.loads(body).get("message", ""))
    except ValueError, AttributeError:
        message = ""
    if status in (403, 429) and headers.get("x-ratelimit-remaining") == "0":
        reset = headers.get("x-ratelimit-reset", "")
        when = time.strftime("%H:%M", time.localtime(int(reset))) if reset.isdigit() else "later"
        return HttpError(
            status,
            f"GitHub API rate limit reached; resets at {when} (local time). --auth raises it.",
        )
    if status in (403, 429) and (after := headers.get("retry-after", "")).isdigit():
        return HttpError(
            status, f"GitHub asked to wait {after} s (secondary rate limit). Try again later."
        )
    if status == 401:
        return HttpError(status, "GitHub rejected the token (401).")
    detail = f": {message}" if message else ""
    if status == 403:
        return HttpError(status, f"GitHub refused the request (403){detail}")
    return HttpError(status, f"GitHub answered {status}{detail}")


def plan(target: Target, *, auth: bool, max_file_size: int) -> Plan:
    """Resolves the ref, lists the tree and selects files, without downloading any but the
    root .gitignore."""
    client = Client(token() if auth else None)
    notes = []
    try:
        info = client.json(f"/repos/{target.name}")
    except HttpError as e:
        if e.status == 404:
            raise RemoteError(
                f"{target.name} not found, or the token cannot read it."
                if auth
                else f"{target.name} not found. If it is private, add --auth."
            ) from None
        raise
    name = str(info["full_name"])
    # The name goes into every later URL, so it must be a name and not a path.
    owner, _, repo = name.partition("/")
    if not _OWNER.fullmatch(owner) or not _REPO.fullmatch(repo) or repo in (".", ".."):
        raise RemoteError(f"GitHub answered with an unexpected repository name for {target.name}.")
    if name.lower() != target.name.lower():
        notes.append(f"{target.name} moved to {name}")
    default = str(info["default_branch"])
    ref = target.ref or default
    try:
        body, _ = client.get(
            f"{client.api}/repos/{name}/commits/{urllib.parse.quote(ref, safe='/')}",
            accept="application/vnd.github.sha",
        )
    except HttpError as e:
        if e.status in (404, 422):
            raise RemoteError(
                f"No branch, tag or commit {ref} in {name} (default branch: {default})."
            ) from None
        raise
    if not _SHA.fullmatch(commit := body.decode("ascii", "replace").strip()):
        raise RemoteError(f"GitHub gave no commit for {ref}.")

    targets = [_split_range(repo_target(p)) for p in target.paths]
    scopes = list(dict.fromkeys(path for path, _ in targets))
    entries = _list(client, name, commit, scopes)
    by_path = {e.path: e for e in entries}
    for path, span in targets:
        if path and path not in by_path:
            raise RemoteError(f"{path} is not in {name} at {ref}.")
        if span and by_path[path].kind != "blob":
            raise RemoteError(f"{path} is not a file in {name}, so {path}{span} is no line range.")

    gitignore = None
    if (e := by_path.get(".gitignore")) and e.kind == "blob" and e.mode != "120000":
        gitignore = _download(client, name, commit, e)
    files, withheld, skipped = _select(entries, gitignore, scopes, max_file_size)
    return Plan(client, name, ref, commit, targets, files, withheld, skipped, notes, gitignore)


def _entries(data: dict, prefix: str = "") -> list[Entry]:
    return [
        Entry(
            f"{prefix}{item['path']}",
            str(item["type"]),
            str(item.get("mode", "")),
            int(item.get("size") or 0),
            str(item["sha"]),
        )
        for item in data.get("tree", [])
    ]


def _list(client: Client, name: str, commit: str, scopes: list[str]) -> list[Entry]:
    """Every entry under the scopes. A recursive listing GitHub had to cut short is replaced by
    a walk down to each scope and a listing of that scope alone."""
    data = client.json(f"/repos/{name}/git/trees/{commit}?recursive=1")
    if not data.get("truncated"):
        return _entries(data)
    # The root is the listing that was cut short, so it cannot be a scope here, even next to others.
    if not scopes or "" in scopes:
        raise RemoteError(f"{name} is too large to list at once; name folders with --path.")
    root = _entries(client.json(f"/repos/{name}/git/trees/{commit}"))
    found = {e.path: e for e in root if e.path == ".gitignore"}
    for scope in scopes:
        level, parts = root, scope.split("/")
        for depth in range(len(parts)):
            here = "/".join(parts[: depth + 1])
            entry = next((e for e in level if e.path == here), None)
            if entry is None:
                break
            found[here] = entry
            if entry.kind != "tree":
                break
            listing = client.json(
                f"/repos/{name}/git/trees/{entry.sha}"
                + ("?recursive=1" if depth == len(parts) - 1 else "")
            )
            if listing.get("truncated"):
                raise RemoteError(f"{scope} in {name} is too large to list; name a smaller folder.")
            level = _entries(listing, f"{here}/")
            if depth == len(parts) - 1:
                found |= {e.path: e for e in level}
    return list(found.values())


def _scopes_of(path: str, scopes: list[str]) -> list[str]:
    """The named paths whose walk reaches this path ("" is the root)."""
    if not scopes:
        return [""]
    return [s for s in scopes if not s or path == s or path.startswith(s + "/")]


def _kept(path: str, scope: str, rules: scanner.ProjectScanner) -> bool:
    """The local scanner's decision: a folder it skips hides everything below it."""
    if path == scope:
        return not rules.is_ignored_rel(path)
    parts = path.split("/")
    first = len(scope.split("/")) if scope else 0
    for depth in range(first + 1, len(parts)):
        if rules.is_ignored_rel("/".join(parts[:depth]), is_dir=True):
            return False
    return not rules.is_ignored_rel(path)


def valid_here(path: str) -> bool:
    """Whether the path can be written under a folder on this system as it is named."""
    parts = path.split("/")
    if any(p in ("", ".", "..") or "\\" in p or "\0" in p for p in parts):
        return False
    if os.name == "nt":
        return not any(
            _RESERVED.match(p) or p[-1] in ". " or any(c in '<>:"|?*' or ord(c) < 32 for c in p)
            for p in parts
        )
    return True


def _select(
    entries: list[Entry], gitignore: bytes | None, scopes: list[str], max_file_size: int
) -> tuple[list[Entry], list[str], dict[str, list[str]]]:
    with tempfile.TemporaryDirectory(prefix="cdg-rules-") as tmp:
        root = Path(tmp)
        if gitignore is not None:
            (root / ".gitignore").write_bytes(gitignore)
        rules = scanner.ProjectScanner(root)

    files, withheld = [], []
    skipped: dict[str, list[str]] = {}
    seen: set[str] = set()
    fold = os.name == "nt" or sys.platform == "darwin"
    for entry in sorted(entries, key=lambda e: e.path):
        # Named paths are walked one by one and the results joined, as the local scanner does.
        if entry.kind == "tree" or not any(
            _kept(entry.path, scope, rules) for scope in _scopes_of(entry.path, scopes)
        ):
            continue
        reason = None
        if entry.kind == "commit":
            reason = "submodule"
        elif entry.mode == "120000":
            reason = "symlink"
        elif is_secret_file(entry.path.rsplit("/", 1)[-1]):
            withheld.append(entry.path)
            continue
        elif not valid_here(entry.path):
            reason = "path not valid on this system"
        elif entry.size > max_file_size:
            reason = f"over {_size_label(max_file_size)}"
        elif (key := entry.path.casefold() if fold else entry.path) in seen:
            reason = "same name as another file except for case"
        else:
            seen.add(key)
            files.append(entry)
            continue
        skipped.setdefault(reason, []).append(entry.path)
    return files, withheld, skipped


def _size_label(size: int) -> str:
    return decimal(size)


def _download(client: Client, name: str, commit: str, entry: Entry) -> bytes:
    if client.token:
        # Blobs by content hash from the API, so the token never goes to another host.
        url = f"{client.api}/repos/{name}/git/blobs/{entry.sha}"
        body, _ = client.get(url, accept="application/vnd.github.raw+json")
    else:
        url = f"{client.raw}/{name}/{commit}/{urllib.parse.quote(entry.path)}"
        body, _ = client.get(url, accept="*/*")
    return body


@contextlib.contextmanager
def materialize(plan: Plan) -> Generator[Path]:
    """The selected files (and the root .gitignore) in a new temporary folder, removed after."""
    root = Path(tempfile.mkdtemp(prefix="cdg-remote-")).resolve()
    try:
        if plan.gitignore is not None:
            (root / ".gitignore").write_bytes(plan.gitignore)
        pending = [
            e for e in plan.files if not (e.path == ".gitignore" and plan.gitignore is not None)
        ]
        with ThreadPoolExecutor(DOWNLOADS) as pool:
            futures = [
                (e, pool.submit(_download, plan.client, plan.name, plan.commit, e)) for e in pending
            ]
            try:
                for entry, future in futures:
                    _write(root, entry.path, future.result())
            except BaseException:
                pool.shutdown(cancel_futures=True)
                raise
        yield root
    finally:
        remove_tree(root)


def _write(root: Path, path: str, data: bytes) -> None:
    target = root.joinpath(*path.split("/"))
    # The listing comes from the network: whatever it names must land inside the folder.
    if not target.resolve().is_relative_to(root):
        raise RemoteError(f"{path} would be written outside the temporary folder.")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
