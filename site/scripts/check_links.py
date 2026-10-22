"""
Fail when a page of the built site links to a page or anchor that does not exist.

Reads the HTML that `npm run build` writes to site/.vitepress/dist, so it sees the
links and heading ids exactly as they are published under the /codigest/ base.

    cd site && npm run build && python scripts/check_links.py
"""

import re
import sys
from html import unescape
from pathlib import Path

DIST = Path(__file__).resolve().parent.parent / ".vitepress" / "dist"
BASE = "/codigest/"

HREF = re.compile(r'<a\b[^>]*?\shref="([^"]*)"')
ID = re.compile(r'\sid="([^"]*)"')


def resolve(path: str) -> Path | None:
    rel = path.removeprefix(BASE)
    for candidate in (DIST / rel, DIST / f"{rel}.html", DIST / rel / "index.html"):
        if candidate.is_file():
            return candidate
    return None


def main() -> int:
    ids: dict[Path, set[str]] = {}
    problems: list[str] = []
    pages = sorted(DIST.rglob("*.html"))
    for page in pages:
        for href in map(unescape, HREF.findall(page.read_text(encoding="utf-8"))):
            if href.startswith(("http://", "https://", "mailto:")):
                continue
            path, _, fragment = href.partition("#")
            if not path:
                target = page
            elif not path.startswith(BASE):
                problems.append(f"{page.relative_to(DIST)}: {href} is outside {BASE}")
                continue
            elif (target := resolve(path)) is None:
                problems.append(f"{page.relative_to(DIST)}: {href} has no page")
                continue
            if fragment:
                if target not in ids:
                    ids[target] = set(ID.findall(target.read_text(encoding="utf-8")))
                if fragment not in ids[target]:
                    problems.append(f"{page.relative_to(DIST)}: {href} has no such anchor")

    if not pages:
        problems.append(f"no HTML under {DIST}; run npm run build first")
    for problem in problems:
        print(problem)
    print(f"{len(pages)} pages, {len(problems)} broken links")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
