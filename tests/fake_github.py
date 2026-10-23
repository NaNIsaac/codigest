"""A fake GitHub for remote scan tests: the API and raw hosts on two local ports, serving one
repository from a folder and recording every request."""

import contextlib
import hashlib
import json
import threading
import urllib.parse
from collections.abc import Generator
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def blob_sha(data: bytes) -> str:
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def fake_sha(text: str) -> str:
    return hashlib.sha1(text.encode()).hexdigest()


@dataclass(slots=True)
class Request:
    method: str
    host: str  # "api" or "raw"
    path: str
    headers: dict[str, str]


@dataclass
class FakeGitHub:
    root: Path
    name: str = "octo/sample"
    default_branch: str = "main"
    # Repository is private: answers 404 unless this token is sent.
    private_token: str | None = None
    # A former name that redirects to `name`.
    moved_from: str | None = None
    truncate: bool = False
    # Listing entries without a file behind them (symlinks, submodules, hostile paths).
    extra: list[dict] = field(default_factory=list)
    # Canned answers, by request path with query, used before the normal one.
    queue: dict[str, list[tuple[int, dict[str, str], bytes]]] = field(default_factory=dict)
    requests: list[Request] = field(default_factory=list)
    api: str = ""
    raw: str = ""

    @property
    def commit(self) -> str:
        return fake_sha(f"{self.name}@commit")

    def files(self) -> dict[str, bytes]:
        return {
            p.relative_to(self.root).as_posix(): p.read_bytes()
            for p in sorted(self.root.rglob("*"))
            if p.is_file()
        }

    def entries(self, folder: str, recursive: bool) -> list[dict]:
        """Tree entries under `folder` ("" for the root), paths relative to it."""
        prefix = f"{folder}/" if folder else ""
        files = {p: d for p, d in self.files().items() if p.startswith(prefix)}
        dirs = {"/".join(p.split("/")[:i]) for p in files for i in range(1, p.count("/") + 1)}
        items = [
            {"path": d, "type": "tree", "mode": "040000", "sha": fake_sha(f"tree:{d}")}
            for d in dirs
            if d.startswith(prefix)
        ]
        items += [
            {"path": p, "type": "blob", "mode": "100644", "size": len(d), "sha": blob_sha(d)}
            for p, d in files.items()
        ]
        if not folder:
            items += self.extra
        items = [{**i, "path": i["path"].removeprefix(prefix)} for i in items]
        if not recursive:
            items = [i for i in items if "/" not in i["path"]]
        return sorted(items, key=lambda i: i["path"])

    def folder_of(self, sha: str) -> str | None:
        if sha in (self.commit, fake_sha("tree:")):
            return ""
        dirs = {
            "/".join(p.split("/")[:i]) for p in self.files() for i in range(1, p.count("/") + 1)
        }
        return next((d for d in dirs if fake_sha(f"tree:{d}") == sha), None)

    def answer(self, host: str, path: str, headers: dict[str, str]) -> tuple[int, dict, bytes]:
        if canned := self.queue.get(path):
            return canned.pop(0)
        parsed = urllib.parse.urlsplit(path)
        parts = [urllib.parse.unquote(p) for p in parsed.path.strip("/").split("/")]
        authorized = self.private_token is None or headers.get("authorization") == (
            f"Bearer {self.private_token}"
        )
        not_found = (404, {}, json.dumps({"message": "Not Found"}).encode())
        if host == "raw":
            owner, repo, commit, *rest = parts
            data = self.files().get("/".join(rest))
            if (
                not authorized
                or f"{owner}/{repo}" != self.name
                or commit != self.commit
                or data is None
            ):
                return not_found
            return 200, {}, data
        if parts[0] != "repos" or len(parts) < 3:
            return not_found
        repo = f"{parts[1]}/{parts[2]}"
        if repo == self.moved_from:
            rest = "/".join(parts[3:])
            target = f"{self.api}/repos/{self.name}" + (f"/{rest}" if rest else "")
            return 301, {"Location": target}, b""
        if repo != self.name or not authorized:
            return not_found
        match parts[3:]:
            case []:
                body = {"full_name": self.name, "default_branch": self.default_branch}
                return 200, {}, json.dumps(body).encode()
            case ["commits", *ref]:
                if "/".join(ref) in (self.default_branch, self.commit):
                    return 200, {}, self.commit.encode()
                return 422, {}, json.dumps({"message": "No commit found"}).encode()
            case ["git", "trees", sha]:
                if (folder := self.folder_of(sha)) is None:
                    return not_found
                recursive = "recursive=1" in parsed.query
                items = self.entries(folder, recursive)
                truncated = self.truncate and recursive and folder == ""
                if truncated:
                    items = items[:1]
                body = {"sha": sha, "tree": items, "truncated": truncated}
                return 200, {}, json.dumps(body).encode()
            case ["git", "blobs", sha]:
                data = next((d for d in self.files().values() if blob_sha(d) == sha), None)
                return not_found if data is None else (200, {}, data)
        return not_found

    def fetched(self) -> list[str]:
        """Repository paths whose content was requested, from either host."""
        files = self.files()
        by_sha = {blob_sha(d): p for p, d in files.items()}
        paths = []
        for r in self.requests:
            parts = [urllib.parse.unquote(p) for p in r.path.split("?")[0].strip("/").split("/")]
            if r.host == "raw":
                paths.append("/".join(parts[3:]))
            elif parts[3:5] == ["git", "blobs"]:
                paths.append(by_sha.get(parts[5], parts[5]))
        return paths

    @contextlib.contextmanager
    def running(self) -> Generator[FakeGitHub]:
        servers = [ThreadingHTTPServer(("127.0.0.1", 0), _handler(self, h)) for h in ("api", "raw")]
        self.api, self.raw = (f"http://127.0.0.1:{s.server_address[1]}" for s in servers)
        threads = [
            threading.Thread(target=s.serve_forever, args=(0.02,), daemon=True) for s in servers
        ]
        for t in threads:
            t.start()
        try:
            yield self
        finally:
            for s in servers:
                s.shutdown()
                s.server_close()


def _handler(fake: FakeGitHub, host: str) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args) -> None:
            pass

        def _serve(self) -> None:
            headers = {k.lower(): v for k, v in self.headers.items()}
            fake.requests.append(Request(self.command, host, self.path, headers))
            if self.command != "GET":
                status, extra, body = 405, {}, b""
            else:
                status, extra, body = fake.answer(host, self.path, headers)
            self.send_response(status)
            for key, value in extra.items():
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = _serve

    return Handler
