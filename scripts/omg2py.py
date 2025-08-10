"""Transpile OMG scripts to Python code.

This module provides a simple transpiler that converts OMG source files
into equivalent Python code by walking the parsed AST.

File: omg2py.py
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © 2025 Chris Rowles. All rights reserved.
Version: 0.1.1
License: MIT
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Iterable

from omglang.interpreter import Interpreter
from omglang.lexer import tokenize
from omglang.parser import Parser
from omglang.operations import Op


class _Transpiler:
    """AST to Python code translator."""

    def __init__(self) -> None:
        self.indent = 0
        self.lines: list[str] = []

    def emit(self, line: str) -> None:
        """Append a line respecting the current indentation."""
        self.lines.append("    " * self.indent + line)

    def transpile(self, ast: Iterable[tuple]) -> str:
        """Transpile a list of statements into Python code."""
        for stmt in ast:
            self._stmt(stmt)
        return "\n".join(self.lines)

    def _stmt(self, node: tuple) -> None:  # noqa: C901 - structured dispatcher
        kind = node[0]
        if kind == "decl":
            _, name, expr, _ = node
            self.emit(f"{name} = {self._expr(expr)}")
        elif kind == "assign":
            _, name, expr, _ = node
            self.emit(f"{name} = {self._expr(expr)}")
        elif kind == "attr_assign":
            _, obj, attr, value, _ = node
            self.emit(f"{self._expr(obj)}.{attr} = {self._expr(value)}")
        elif kind == "index_assign":
            _, obj, index, value, _ = node
            self.emit(f"{self._expr(obj)}[{self._expr(index)}] = {self._expr(value)}")
        elif kind == "emit":
            _, expr, _ = node
            self.emit(f"print({self._expr(expr)})")
        elif kind == "facts":
            _, expr, _ = node
            self.emit(f"assert {self._expr(expr)}")
        elif kind == "if":
            self._if(node, initial=True)
        elif kind == "loop":
            _, cond, body, _ = node
            self.emit(f"while {self._expr(cond)}:")
            self._block(body)
        elif kind == "break":
            self.emit("break")
        elif kind == "func_def":
            _, name, params, body, _ = node
            self.emit(f"def {name}({', '.join(params)}):")
            self._block(body)
        elif kind == "return":
            _, expr, _ = node
            self.emit(f"return {self._expr(expr)}")
        elif kind == "expr_stmt":
            _, expr, _ = node
            self.emit(self._expr(expr))
        elif kind == "import":
            _, path, alias, _ = node
            module = self._module_from_path(path)
            self.emit(f"import {module} as {alias}")
        elif kind == "try":
            _, try_block, err_name, except_block, _ = node
            self.emit("try:")
            self._block(try_block)
            if err_name:
                self.emit(f"except Exception as {err_name}:")
            else:
                self.emit("except Exception:")
            self._block(except_block)
        elif kind == "block":
            self._block(node)
        else:
            self.emit(f"# Unsupported statement: {kind}")

    def _block(self, block_node: tuple) -> None:
        """Transpile a block node."""
        _, statements, _ = block_node
        self.indent += 1
        if statements:
            for stmt in statements:
                self._stmt(stmt)
        else:
            self.emit("pass")
        self.indent -= 1

    def _if(self, node: tuple, initial: bool) -> None:
        """Transpile an if/elif/else chain."""
        _, cond, then_block, else_block, _ = node
        keyword = "if" if initial else "elif"
        self.emit(f"{keyword} {self._expr(cond)}:")
        self._block(then_block)
        if else_block:
            if else_block[0] == "if":
                self._if(else_block, initial=False)
            else:
                self.emit("else:")
                self._block(else_block)

    def _expr(self, node: tuple) -> str:  # noqa: C901 - recursive cases
        kind = node[0]
        if isinstance(kind, Op):
            left = self._expr(node[1])
            right = self._expr(node[2])
            op_map = {
                Op.ADD: "+",
                Op.SUB: "-",
                Op.MUL: "*",
                Op.DIV: "/",
                Op.MOD: "%",
                Op.AND_BITS: "&",
                Op.OR_BITS: "|",
                Op.XOR_BITS: "^",
                Op.SHL: "<<",
                Op.SHR: ">>",
                Op.EQ: "==",
                Op.NE: "!=",
                Op.GT: ">",
                Op.LT: "<",
                Op.GE: ">=",
                Op.LE: "<=",
                Op.AND: "and",
                Op.OR: "or",
            }
            return f"({left} {op_map[kind]} {right})"
        if kind == "unary":
            _, op, operand, _ = node
            op_map = {Op.ADD: "+", Op.SUB: "-", Op.NOT_BITS: "~"}
            return f"({op_map[op]}{self._expr(operand)})"
        if kind == "number":
            return str(node[1])
        if kind == "string":
            return repr(node[1])
        if kind == "bool":
            return "True" if node[1] else "False"
        if kind == "ident":
            return node[1]
        if kind == "list":
            return "[" + ", ".join(self._expr(e) for e in node[1]) + "]"
        if kind == "dict":
            return "{" + ", ".join(f"{repr(k)}: {self._expr(v)}" for k, v in node[1]) + "}"
        if kind == "func_call":
            func = self._expr(node[1])
            args = ", ".join(self._expr(a) for a in node[2])
            return f"{func}({args})"
        if kind == "index":
            return f"{self._expr(node[1])}[{self._expr(node[2])}]"
        if kind == "slice":
            seq = self._expr(node[1])
            start = self._expr(node[2])
            end = self._expr(node[3]) if node[3] is not None else ""
            return f"{seq}[{start}:{end}]"
        if kind == "dot":
            return f"{self._expr(node[1])}.{node[2]}"
        return "None"

    @staticmethod
    def _module_from_path(path: str) -> str:
        """Convert an import path to a Python module path."""
        mod = Path(path).with_suffix("")
        parts = [p for p in mod.parts if p not in {".", ""}]
        return ".".join(parts)


def transpile_omg_to_py(source: str, file: str) -> str:
    """Transpile OMG source code to Python code."""
    interpreter = Interpreter(file)
    interpreter.check_header(source)
    tokens, token_map = tokenize(source)
    parser = Parser(tokens, token_map, file)
    ast = parser.parse()
    return _Transpiler().transpile(ast)


def transpile_file(src: str, out: str) -> None:
    """Transpile an OMG file to Python and write the result."""
    source = Path(src).read_text(encoding="utf-8")
    python_code = transpile_omg_to_py(source, src)
    Path(out).write_text(python_code, encoding="utf-8")


def main() -> None:
    """CLI entry point for OMG→Python transpilation."""
    parser = argparse.ArgumentParser(description="Transpile an OMG script to Python")
    parser.add_argument("src", help="Path to .omg source script")
    parser.add_argument("-o", "--out", dest="out", default=None, help="Output .py file path")
    args = parser.parse_args()
    out_path = args.out or str(Path(args.src).with_suffix(".py"))
    transpile_file(args.src, out_path)


if __name__ == "__main__":
    main()
