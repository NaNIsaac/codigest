"""Compare the Python and Rust Python outlines file by file.

    cargo build --example outline
    python rust/scripts/outline_parity.py src rust/target/debug/examples/outline DIR...

The first argument is the folder holding the `codigest` package (the reference build).
"""
import difflib
import pathlib
import subprocess
import sys

sys.path.insert(0, sys.argv[1])
from codigest.core import semdiff

exe, roots = sys.argv[2], sys.argv[3:]
skip = {".venv", "node_modules", ".git"}
files = [p for r in roots for p in pathlib.Path(r).rglob("*.py") if not skip & set(p.parts)]
mismatches, total = [], 0
for start in range(0, len(files), 200):
    batch = files[start : start + 200]
    out = subprocess.run([exe, *map(str, batch)], capture_output=True, text=True, encoding="utf-8", check=False).stdout
    for path, part in zip(batch, out.split("=== ")[1:], strict=True):
        rust = part.split("\n", 1)[1].rstrip("\n") if "\n" in part else ""
        try:
            python = semdiff.summarize(path.read_text(encoding="utf-8"))
        except UnicodeDecodeError:
            continue
        total += 1
        if python != rust:
            mismatches.append((path, python, rust))

print(f"{total} files, {len(mismatches)} differ")
for path, python, rust in mismatches[:20]:
    diff = difflib.unified_diff(python.splitlines(), rust.splitlines(), "python", "rust", lineterm="", n=0)
    print(f"--- {path}\n" + "\n".join(list(diff)[2:8]))
sys.exit(1 if mismatches else 0)
