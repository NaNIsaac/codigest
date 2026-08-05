"""
Git Blob Hash-based Content & AST Cache.
Provides fast caching for scan snapshot blocks and digest AST summaries.
"""
import json
import hashlib
from pathlib import Path
from typing import Optional, Dict, Any

def compute_git_blob_hash(data: bytes) -> str:
    """Computes Git-compatible SHA-1 blob hash for raw bytes."""
    header = f"blob {len(data)}\0".encode("utf-8")
    return hashlib.sha1(header + data).hexdigest()

def get_file_blob_hash(file_path: Path) -> Optional[str]:
    """Reads file and computes its Git blob hash."""
    try:
        data = file_path.read_bytes()
        return compute_git_blob_hash(data)
    except Exception:
        return None

class ContentCache:
    def __init__(self, root_path: Path):
        self.root_path = root_path
        self.cache_file = root_path / ".codigest" / "cache.json"
        self._data: Dict[str, Any] = {}
        self.hits = 0
        self.misses = 0
        self._load()

    def _load(self):
        if self.cache_file.exists():
            try:
                with open(self.cache_file, "r", encoding="utf-8") as f:
                    self._data = json.load(f)
            except Exception:
                self._data = {}
        else:
            self._data = {}

    def save(self):
        try:
            self.cache_file.parent.mkdir(parents=True, exist_ok=True)
            with open(self.cache_file, "w", encoding="utf-8") as f:
                json.dump(self._data, f, ensure_ascii=False, indent=2)
        except Exception:
            pass

    def get_file_block(self, rel_path: str, blob_hash: str, line_numbers: bool) -> Optional[str]:
        cache_key = f"{rel_path}:{blob_hash}:lines={line_numbers}"
        val = self._data.get("file_blocks", {}).get(cache_key)
        if val is not None:
            self.hits += 1
        else:
            self.misses += 1
        return val

    def set_file_block(self, rel_path: str, blob_hash: str, line_numbers: bool, block: str):
        if "file_blocks" not in self._data:
            self._data["file_blocks"] = {}
        cache_key = f"{rel_path}:{blob_hash}:lines={line_numbers}"
        self._data["file_blocks"][cache_key] = block

    def get_summary(self, rel_path: str, blob_hash: str) -> Optional[str]:
        cache_key = f"{rel_path}:{blob_hash}"
        val = self._data.get("summaries", {}).get(cache_key)
        if val is not None:
            self.hits += 1
        else:
            self.misses += 1
        return val

    def set_summary(self, rel_path: str, blob_hash: str, summary: str):
        if "summaries" not in self._data:
            self._data["summaries"] = {}
        cache_key = f"{rel_path}:{blob_hash}"
        self._data["summaries"][cache_key] = summary
