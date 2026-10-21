"""The project code a failing test exercises.

A traceback that ends in an assertion names the test, not the function under
test. When that function is unchanged (the bug is committed, or the baseline
moved past it), nothing else in a pack shows it. The calls made by the failing
test functions, matched against the definitions in the project modules the test
imports, recover it.
"""

import ast
import warnings
from pathlib import Path

MAX_DEFINITIONS = 8


def _parse(path: Path) -> ast.Module | None:
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", SyntaxWarning)
            return ast.parse(path.read_text(encoding="utf-8"))
    except OSError, UnicodeDecodeError, SyntaxError, ValueError:
        return None


def _enclosing_functions(tree: ast.Module, lines: list[int]) -> list[ast.AST]:
    found = []
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef) and any(
            node.lineno <= line <= (node.end_lineno or node.lineno) for line in lines
        ):
            found.append(node)
    return found


def _called_names(nodes: list[ast.AST]) -> list[str]:
    names: list[str] = []
    for node in nodes:
        for call in ast.walk(node):
            if not isinstance(call, ast.Call):
                continue
            match call.func:
                case ast.Name(id=name) | ast.Attribute(attr=name):
                    if name not in names:
                        names.append(name)
    return names


def _definitions(tree: ast.Module) -> dict[str, tuple[int, int]]:
    """Top-level functions and classes, and methods by their bare name, to (first, last) line."""
    spans: dict[str, tuple[int, int]] = {}
    for node in tree.body:
        if isinstance(node, ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef):
            first = min([node.lineno, *(d.lineno for d in node.decorator_list)])
            spans[node.name] = (first, node.end_lineno or node.lineno)
        if isinstance(node, ast.ClassDef):
            for item in node.body:
                if isinstance(item, ast.FunctionDef | ast.AsyncFunctionDef):
                    first = min([item.lineno, *(d.lineno for d in item.decorator_list)])
                    spans.setdefault(item.name, (first, item.end_lineno or item.lineno))
    return spans


def called_definitions(
    test: Path, lines: list[int], modules: set[Path]
) -> list[tuple[Path, str, tuple[int, int]]]:
    """(file, name, line span) of definitions in `modules` called by the test functions that
    contain `lines`, in call order, at most MAX_DEFINITIONS."""
    if not (tree := _parse(test)):
        return []
    called = _called_names(_enclosing_functions(tree, lines))
    defined = {path: _definitions(t) for path in sorted(modules) if (t := _parse(path))}
    found = []
    for name in called:
        for path, spans in defined.items():
            if name in spans:
                found.append((path, name, spans[name]))
                break
        if len(found) == MAX_DEFINITIONS:
            break
    return found
