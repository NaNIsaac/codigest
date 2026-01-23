"""
Semantic Difference Engine (Spec 0.1 Compliant).
Tracks: Functions, Classes, Methods, Global Variables.
Detects: Signature Changes AND Logic Body Changes.
"""
import ast
import hashlib
from dataclasses import dataclass
from typing import Dict, List, Optional, Any

@dataclass
class SymbolInfo:
    name: str
    type: str  # 'function', 'class', 'variable', ...
    signature: str
    content_hash: str # For logic change detection
    docstring: Optional[str] = None

@dataclass
class SemanticChange:
    change_type: str  # 'ADDED', 'REMOVED', 'MODIFIED', 'LOGIC_CHANGED'
    symbol: SymbolInfo
    details: str = ""

class CodeParser(ast.NodeVisitor):
    def __init__(self):
        self.symbols: dict[str, SymbolInfo] = {}
        self.current_class = None

    def _get_hash(self, node: ast.AST) -> str:
        """Normalized hash of the node body (ignoring formatting/comments implicitly by unparse)."""
        try:
            # ast.unparse returns standardized code string
            normalized_code = ast.unparse(node)
            return hashlib.md5(normalized_code.encode('utf-8')).hexdigest()
        except Exception:
            return ""

    def _get_signature(self, node) -> str:
        """Reconstructs signature."""
        if hasattr(node, 'args'):
            try:
                return f"({ast.unparse(node.args)})"
            except: pass
        return ""

    def visit_FunctionDef(self, node):
        self._visit_func(node, "function")

    def visit_AsyncFunctionDef(self, node):
        self._visit_func(node, "async_function")

    def _visit_func(self, node, type_label):
        name = node.name
        if self.current_class:
            name = f"{self.current_class}.{name}"
            type_label = "method"
        
        self.symbols[name] = SymbolInfo(
            name=name,
            type=type_label,
            signature=self._get_signature(node),
            content_hash=self._get_hash(node), # Hash entire function body
            docstring=ast.get_docstring(node)
        )

    def visit_ClassDef(self, node):
        prev_class = self.current_class
        self.current_class = node.name
        
        bases = [ast.unparse(b) for b in node.bases]
        base_str = f"({', '.join(bases)})" if bases else ""
        
        self.symbols[node.name] = SymbolInfo(
            name=node.name,
            type="class",
            signature=base_str,
            content_hash=self._get_hash(node), # Hash class structure
            docstring=ast.get_docstring(node)
        )
        
        self.generic_visit(node)
        self.current_class = prev_class

    def visit_Assign(self, node):
        # Only track global variables (no indent)
        if self.current_class is None:
            for target in node.targets:
                if isinstance(target, ast.Name):
                    name = target.id
                    # Try to infer value type/content roughly
                    try:
                        val_str = ast.unparse(node.value)
                        # Truncate long values
                        if len(val_str) > 50: val_str = val_str[:47] + "..."
                        sig = f" = {val_str}"
                    except:
                        sig = " = ..."
                        
                    self.symbols[name] = SymbolInfo(
                        name=name,
                        type="variable",
                        signature=sig,
                        content_hash=self._get_hash(node)
                    )
    
    # Python 3.6+ Annotated Assignment (x: int = 1)
    def visit_AnnAssign(self, node):
        if self.current_class is None and isinstance(node.target, ast.Name):
            name = node.target.id
            try:
                type_hint = ast.unparse(node.annotation)
                val_str = f" = {ast.unparse(node.value)}" if node.value else ""
                sig = f": {type_hint}{val_str}"
            except:
                sig = ": ..."
                
            self.symbols[name] = SymbolInfo(
                name=name,
                type="variable",
                signature=sig,
                content_hash=self._get_hash(node)
            )

def parse_code(code: str) -> dict[str, SymbolInfo]:
    if not code: return {}
    try:
        tree = ast.parse(code)
        parser = CodeParser()
        parser.visit(tree)
        return parser.symbols
    except SyntaxError:
        return {} 

def compare(old_code: str, new_code: str) -> list[SemanticChange]:
    old_syms = parse_code(old_code)
    new_syms = parse_code(new_code)
    
    changes = []
    all_keys = set(old_syms.keys()) | set(new_syms.keys())
    
    for key in sorted(all_keys):
        if key not in old_syms:
            changes.append(SemanticChange("ADDED", new_syms[key]))
        elif key not in new_syms:
            changes.append(SemanticChange("REMOVED", old_syms[key]))
        else:
            old, new = old_syms[key], new_syms[key]
            
            # 1. Signature Change? (API Breakage)
            if old.signature != new.signature:
                detail = f"{old.signature} -> {new.signature}"
                changes.append(SemanticChange("MODIFIED", new, detail))
            
            # 2. Logic Change? (Internal Implementation)
            # Only check if signature didn't change (to avoid double reporting)
            elif old.content_hash != new.content_hash:
                changes.append(SemanticChange("LOGIC_CHANGED", new, "Implementation changed"))
            
    return changes

def summarize(code: str) -> str:
    symbols = parse_code(code)
    if not symbols: return ""
    
    lines = []
    for name, sym in sorted(symbols.items()):
        if sym.type == "class": icon = "📦"
        elif sym.type == "method": icon = "  ƒ"
        elif sym.type == "async_function": icon = "⚡"
        elif sym.type == "variable": icon = "🔹"
        else: icon = "ƒ"
        
        lines.append(f"{icon} {sym.type} {sym.name}{sym.signature}")
    return "\n".join(lines)