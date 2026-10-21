"""
Semantic Difference Engine (Spec 0.1 Compliant).
Tracks: Functions, Classes, Methods, Global Variables.
Detects: Signature Changes AND Logic Body Changes.
"""
import ast
import warnings
lazy import hashlib
from dataclasses import dataclass
from typing import Literal

type SymbolKind = Literal["function", "async_function", "method", "class", "variable", "attribute"]
type ChangeType = Literal["ADDED", "REMOVED", "MODIFIED", "LOGIC_CHANGED"]

# Longest value shown for a variable or attribute in an outline.
VALUE_WIDTH = 60

@dataclass(slots=True)
class SymbolInfo:
    name: str
    type: SymbolKind
    signature: str
    content_hash: str # For logic change detection
    docstring: str | None = None
    line: int = 0

@dataclass(slots=True)
class SemanticChange:
    change_type: ChangeType
    symbol: SymbolInfo
    details: str = ""

class CodeParser(ast.NodeVisitor):
    def __init__(self):
        self.symbols: dict[str, SymbolInfo] = {}
        self.current_class: str | None = None

    def _get_hash(self, node: ast.AST) -> str:
        """Normalized hash of the node body (ignoring formatting/comments implicitly by unparse)."""
        try:
            # ast.unparse returns standardized code string
            normalized_code = ast.unparse(node)
            return hashlib.md5(normalized_code.encode('utf-8')).hexdigest()
        except Exception:
            return ""

    def _get_signature(self, node: ast.AST) -> str:
        """Reconstructs signature."""
        if (args := getattr(node, "args", None)) is not None:
            try:
                ret = getattr(node, "returns", None)
                returns = f" -> {ast.unparse(ret)}" if ret else ""
                return f"({ast.unparse(args)}){returns}"
            except Exception:
                pass
        return ""

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self._visit_func(node, "function")

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self._visit_func(node, "async_function")

    def _visit_func(self, node: ast.FunctionDef | ast.AsyncFunctionDef, type_label: SymbolKind) -> None:
        name = node.name
        if self.current_class:
            name = f"{self.current_class}.{name}"
            type_label = "method"

        self.symbols[name] = SymbolInfo(
            name=name,
            type=type_label,
            signature=self._get_signature(node),
            content_hash=self._get_hash(node), # Hash entire function body
            docstring=ast.get_docstring(node),
            line=node.lineno,
        )

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        prev_class = self.current_class
        self.current_class = node.name

        bases = [ast.unparse(b) for b in node.bases]
        base_str = f"({', '.join(bases)})" if bases else ""

        self.symbols[node.name] = SymbolInfo(
            name=node.name,
            type="class",
            signature=base_str,
            content_hash=self._get_hash(node), # Hash class structure
            docstring=ast.get_docstring(node),
            line=node.lineno,
        )

        self.generic_visit(node)
        self.current_class = prev_class

    def _value(self, node: ast.expr | None) -> str:
        if node is None:
            return ""
        try:
            text = ast.unparse(node)
        except Exception:
            return " = ..."
        return f" = {text if len(text) <= VALUE_WIDTH else text[: VALUE_WIDTH - 3] + '...'}"

    def _add_variable(self, name: str, sig: str, node: ast.stmt) -> None:
        # Class-level names are where tables and dataclass fields live, so they count too.
        kind: SymbolKind = "variable"
        if self.current_class is not None:
            name, kind = f"{self.current_class}.{name}", "attribute"
        self.symbols[name] = SymbolInfo(
            name=name, type=kind, signature=sig, content_hash=self._get_hash(node), line=node.lineno
        )

    def visit_Assign(self, node: ast.Assign) -> None:
        for target in node.targets:
            if isinstance(target, ast.Name):
                self._add_variable(target.id, self._value(node.value), node)

    # Annotated Assignment (x: int = 1)
    def visit_AnnAssign(self, node: ast.AnnAssign) -> None:
        if isinstance(node.target, ast.Name):
            try:
                type_hint = ast.unparse(node.annotation)
            except Exception:
                type_hint = "..."
            self._add_variable(node.target.id, f": {type_hint}{self._value(node.value)}", node)

def parse_code(code: str) -> dict[str, SymbolInfo]:
    if not code:
        return {}
    try:
        # Invalid escapes in someone else's code are their problem, not noise in our output.
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", SyntaxWarning)
            tree = ast.parse(code)
    except SyntaxError, ValueError:
        return {}
    parser = CodeParser()
    parser.visit(tree)
    return parser.symbols

def compare(old_code: str, new_code: str) -> list[SemanticChange]:
    old_syms = parse_code(old_code)
    new_syms = parse_code(new_code)

    changes = []
    for key in sorted(old_syms.keys() | new_syms.keys()):
        match old_syms.get(key), new_syms.get(key):
            case None, new:
                changes.append(SemanticChange("ADDED", new))
            case old, None:
                changes.append(SemanticChange("REMOVED", old))
            # 1. Signature Change? (API Breakage)
            case old, new if old.signature != new.signature:
                changes.append(SemanticChange("MODIFIED", new, f"{old.signature} -> {new.signature}"))
            # 2. Logic Change? (Internal Implementation)
            # Only check if signature didn't change (to avoid double reporting)
            case old, new if old.content_hash != new.content_hash:
                changes.append(SemanticChange("LOGIC_CHANGED", new, "Implementation changed"))

    # A class hash covers its members, so a class whose members are all that changed adds nothing.
    changed = {c.symbol.name for c in changes}
    return [
        c for c in changes
        if not (
            c.symbol.type == "class"
            and c.change_type == "LOGIC_CHANGED"
            and any(name.startswith(f"{c.symbol.name}.") for name in changed)
        )
    ]

DOC_WIDTH = 100


def _first_line(doc: str) -> str:
    line = doc.strip().splitlines()[0].strip()
    return line if len(line) <= DOC_WIDTH else line[: DOC_WIDTH - 3] + "..."


def _module_docstring(code: str) -> tuple[int, str] | None:
    """Line and text of the module docstring, which says what the file is for."""
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", SyntaxWarning)
            tree = ast.parse(code)
    except SyntaxError, ValueError:
        return None
    if (doc := ast.get_docstring(tree)) and doc.strip():
        return tree.body[0].lineno, doc
    return None


def summarize(code: str) -> str:
    """One line per definition in source order, numbered like `scan -l` so a reader can
    jump to it, with the first line of its docstring underneath. The module docstring
    comes first."""
    symbols = parse_code(code)
    module_doc = _module_docstring(code)
    if not symbols and not module_doc:
        return ""

    numbers = [sym.line for sym in symbols.values()] + ([module_doc[0]] if module_doc else [])
    width = len(str(max(numbers)))
    lines = []
    if module_doc:
        lines.append(f'{module_doc[0]:>{width}}: """{_first_line(module_doc[1])}"""')
    for sym in sorted(symbols.values(), key=lambda s: s.line):
        depth = sym.name.count(".") if sym.type in ("method", "attribute") else 0
        indent = "  " * depth
        name = sym.name.rsplit(".", 1)[-1]
        match sym.type:
            case "class":
                head = f"class {name}{sym.signature}"
            case "variable" | "attribute":
                head = f"{name}{sym.signature}"
            case "async_function":
                head = f"async def {name}{sym.signature}"
            case _:
                head = f"def {name}{sym.signature}"
        lines.append(f"{sym.line:>{width}}: {indent}{head}")
        if sym.docstring:
            lines.append(f"{' ' * width}  {indent}    {_first_line(sym.docstring)}")
    return "\n".join(lines)
