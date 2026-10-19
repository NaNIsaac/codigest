"""Outlines for `digest`: one numbered line per definition, bodies left out."""
import re

from . import semdiff

# `function out = name(a, b)`, `function [a, b] = name(x)`, `function name`
_MATLAB_DEF = re.compile(
    r"^\s*(?P<kind>function|classdef)\b\s*(?:\[?[\w\s,~]*\]?\s*=\s*)?(?P<rest>[\w.]+\s*(?:\([^)]*\))?)"
)
_MATLAB_COMMENT = re.compile(r"^\s*%\s?(?P<text>.*\S)")
# Cell titles are how a MATLAB script is organised, and often all a lab script has.
# MATLAB needs the space: `%%%%` banners and `%%comment` are not sections.
_MATLAB_SECTION = re.compile(r"^\s*%% +\S")

SUFFIXES = {".py", ".pyi", ".m"}


def matlab(code: str) -> str:
    """Functions and classdefs with the help line MATLAB shows for them (the first comment),
    and `%%` section titles."""
    lines = code.splitlines()
    width = len(str(len(lines)))
    out = []
    for i, line in enumerate(lines):
        if _MATLAB_SECTION.match(line):
            # A title wrapped over several `%%` lines is one section.
            if not (i and _MATLAB_SECTION.match(lines[i - 1])):
                out.append(f"{i + 1:>{width}}: {line.strip()}")
            continue
        if not _MATLAB_DEF.match(line):
            continue
        out.append(f"{i + 1:>{width}}: {line.strip()}")
        if i + 1 < len(lines) and (c := _MATLAB_COMMENT.match(lines[i + 1])):
            out.append(f"{' ' * width}      {c['text']}")
    return "\n".join(out)


def summarize(suffix: str, code: str) -> str:
    match suffix:
        case ".m":
            return matlab(code)
        case ".py" | ".pyi":
            return semdiff.summarize(code)
    return ""
