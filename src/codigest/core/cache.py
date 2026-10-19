"""
Git Blob Hash-based Content & AST Cache.
Provides fast caching for scan snapshot blocks and digest AST summaries.
"""
lazy import json
lazy import hashlib
from pathlib import Path
from typing import Any

def compute_git_blob_hash(data: bytes) -> str:
    """Computes Git-compatible SHA-1 blob hash for raw bytes."""
    header = f"blob {len(data)}\0".encode("utf-8")
    return hashlib.sha1(header + data).hexdigest()

def get_file_blob_hash(file_path: Path) -> str | None:
    """Reads file and computes its Git blob hash."""
    try:
        return compute_git_blob_hash(file_path.read_bytes())
    except Exception:
        return None

# Bump when the shape of a cached block or summary changes, so stale ones are dropped.
FORMAT = 2


class ContentCache:
    def __init__(self, root_path: Path):
        self.root_path = root_path
        self.cache_file = root_path / ".codigest" / "cache.json"
        self._data: dict[str, Any] = {}
        self.hits = 0
        self.misses = 0
        self._load()

    def _load(self) -> None:
        self._data = {}
        if self.cache_file.exists():
            try:
                with open(self.cache_file, "r", encoding="utf-8") as f:
                    self._data = json.load(f)
            except Exception:
                self._data = {}
        if self._data.get("format") != FORMAT:
            self._data = {"format": FORMAT}

    def save(self) -> None:
        # Only in projects that already have .codigest/ (init or mark): reading must not
        # leave a folder behind in a project that never asked for one.
        if not self.cache_file.parent.is_dir():
            return
        try:
            with open(self.cache_file, "w", encoding="utf-8") as f:
                json.dump(self._data, f, ensure_ascii=False, indent=2)
        except Exception:
            pass

    def get_file_block(
        self,
        rel_path: str,
        blob_hash: str | None = None,
        line_numbers: bool = False,
        file_path: Path | None = None
    ) -> str | None:
        # [1] Fast path: Stat check if file_path is provided
        if file_path is not None:
            try:
                stat = file_path.stat()
                entry = self._data.get("files", {}).get(rel_path)
                if entry and entry.get("mtime_ns") == stat.st_mtime_ns and entry.get("size") == stat.st_size:
                    if (block := entry.get("blocks", {}).get(str(line_numbers))) is not None:
                        self.hits += 1
                        return block
            except Exception:
                pass

        # [2] Hash path (if blob_hash available)
        if blob_hash:
            entry = self._data.get("files", {}).get(rel_path)
            if entry and entry.get("blob_hash") == blob_hash:
                if (block := entry.get("blocks", {}).get(str(line_numbers))) is not None:
                    self.hits += 1
                    if file_path is not None:
                        try:
                            stat = file_path.stat()
                            entry["mtime_ns"] = stat.st_mtime_ns
                            entry["size"] = stat.st_size
                        except Exception:
                            pass
                    return block

        self.misses += 1
        return None

    def set_file_block(
        self,
        rel_path: str,
        blob_hash: str | None,
        line_numbers: bool,
        block: str,
        file_path: Path | None = None
    ) -> None:
        entry = self._data.setdefault("files", {}).setdefault(rel_path, {"blocks": {}})
        if blob_hash:
            entry["blob_hash"] = blob_hash

        if file_path is not None:
            try:
                stat = file_path.stat()
                entry["mtime_ns"] = stat.st_mtime_ns
                entry["size"] = stat.st_size
            except Exception:
                pass

        entry.setdefault("blocks", {})[str(line_numbers)] = block

    def get_summary(self, rel_path: str, blob_hash: str, file_path: Path | None = None) -> str | None:
        if file_path is not None:
            try:
                stat = file_path.stat()
                entry = self._data.get("summaries_meta", {}).get(rel_path)
                if entry and entry.get("mtime_ns") == stat.st_mtime_ns and entry.get("size") == stat.st_size:
                    if (summary := entry.get("summary")) is not None:
                        self.hits += 1
                        return summary
            except Exception:
                pass

        val = self._data.get("summaries", {}).get(f"{rel_path}:{blob_hash}")
        if val is not None:
            self.hits += 1
        else:
            self.misses += 1
        return val

    def set_summary(self, rel_path: str, blob_hash: str, summary: str, file_path: Path | None = None) -> None:
        self._data.setdefault("summaries", {})[f"{rel_path}:{blob_hash}"] = summary

        if file_path is not None:
            meta = self._data.setdefault("summaries_meta", {})
            try:
                stat = file_path.stat()
                meta[rel_path] = {
                    "mtime_ns": stat.st_mtime_ns,
                    "size": stat.st_size,
                    "blob_hash": blob_hash,
                    "summary": summary
                }
            except Exception:
                pass
