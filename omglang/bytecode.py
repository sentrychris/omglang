"""OMG bytecode compiler.

This module compiles OMG source code into a textual bytecode format consumed by the
native Rust virtual machine.  It relies on the existing lexer and parser to produce
an AST and then lowers that tree into a simple stack-based instruction set.

Usage:
    python -m omglang.bytecode path/to/script.omg > script.bc
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import List, Tuple

from omglang.lexer import tokenize
from omglang.parser import Parser
from omglang.operations import Op


Instr = Tuple[str, object | None]


@dataclass
class FunctionEntry:
    name: str
    params: List[str]
    address: int


class Compiler:
    """Compile OMG AST nodes into bytecode instructions."""

    def __init__(self) -> None:
        self.code: List[Instr] = []
        self.pending_funcs: List[Tuple[str, List[str], List[Instr]]] = []
        self.funcs: List[FunctionEntry] = []

    # ------------------------------------------------------------------
    # Helper utilities
    # ------------------------------------------------------------------
    def emit(self, op: str, arg: object | None = None) -> None:
        self.code.append((op, arg))

    def emit_placeholder(self, op: str) -> int:
        idx = len(self.code)
        self.code.append((op, None))
        return idx

    def patch(self, idx: int, target: int) -> None:
        op, _ = self.code[idx]
        self.code[idx] = (op, target)

    # ------------------------------------------------------------------
    # Compilation entry points
    # ------------------------------------------------------------------
    def compile(self, ast: List[tuple]) -> str:
        for stmt in ast:
            self.compile_stmt(stmt)
        self.emit("HALT")
        # Append function bodies and record their addresses
        final_code = list(self.code)
        for name, params, body in self.pending_funcs:
            addr = len(final_code)
            self.funcs.append(FunctionEntry(name, params, addr))
            for op, arg in body:
                if op in {"JUMP", "JUMP_IF_FALSE"} and isinstance(arg, int):
                    final_code.append((op, arg + addr))
                else:
                    final_code.append((op, arg))
        lines: List[str] = []
        for f in self.funcs:
            params = " ".join(f.params)
            lines.append(f"FUNC {f.name} {len(f.params)} {params} {f.address}")
        for op, arg in final_code:
            if arg is None:
                lines.append(op)
            else:
                lines.append(f"{op} {arg}")
        return "\n".join(lines)

    # ------------------------------------------------------------------
    # Statement compilation
    # ------------------------------------------------------------------
    def compile_block(self, block: List[tuple]) -> None:
        for stmt in block:
            self.compile_stmt(stmt)

    def compile_stmt(self, stmt: tuple) -> None:
        kind = stmt[0]
        if kind == "emit":
            self.compile_expr(stmt[1])
            self.emit("EMIT")
        elif kind == "decl" or kind == "assign":
            name, expr = stmt[1], stmt[2]
            self.compile_expr(expr)
            self.emit("STORE", name)
        elif kind == "if":
            # Unroll nested if/elif chain
            cond_blocks: List[Tuple[tuple, List[tuple]]] = []
            else_block: List[tuple] | None = None
            current = stmt
            while True:
                cond = current[1]
                block_node = current[2]
                block_stmts = block_node[1] if block_node[0] == "block" else []
                cond_blocks.append((cond, block_stmts))
                tail = current[3]
                if tail and isinstance(tail, tuple) and tail[0] == "if":
                    current = tail
                else:
                    if tail:
                        else_block = tail[1] if tail[0] == "block" else None
                    break
            end_jumps: List[int] = []
            for cond, block in cond_blocks:
                self.compile_expr(cond)
                jf = self.emit_placeholder("JUMP_IF_FALSE")
                self.compile_block(block)
                end_jumps.append(self.emit_placeholder("JUMP"))
                self.patch(jf, len(self.code))
            if else_block:
                self.compile_block(else_block)
            for j in end_jumps:
                self.patch(j, len(self.code))
        elif kind == "loop":
            cond, body_node = stmt[1], stmt[2]
            body = body_node[1] if body_node[0] == "block" else []
            start = len(self.code)
            self.compile_expr(cond)
            jf = self.emit_placeholder("JUMP_IF_FALSE")
            self.compile_block(body)
            self.emit("JUMP", start)
            self.patch(jf, len(self.code))
        elif kind == "func_def":
            name, params, body_node = stmt[1], stmt[2], stmt[3]
            body = body_node[1] if body_node[0] == "block" else []
            body_code = self._compile_function_body(body)
            self.pending_funcs.append((name, params, body_code))
        elif kind == "return":
            expr = stmt[1]
            if (
                isinstance(expr, tuple)
                and expr[0] == "func_call"
                and expr[1][0] == "ident"
            ):
                func_node, args = expr[1], expr[2]
                for arg in args:
                    self.compile_expr(arg)
                self.emit("TCALL", func_node[1])
            else:
                self.compile_expr(expr)
                self.emit("RET")
        elif kind == "block":
            self.compile_block(stmt[1])
        else:
            raise NotImplementedError(f"Unsupported statement: {stmt}")

    def _compile_function_body(self, body: List[tuple]) -> List[Instr]:
        saved_code = self.code
        self.code = []
        self.compile_block(body)
        self.emit("RET")
        func_code = self.code
        self.code = saved_code
        return func_code

    # ------------------------------------------------------------------
    # Expression compilation
    # ------------------------------------------------------------------
    def compile_expr(self, node: tuple) -> None:
        op = node[0]
        if op == "number":
            self.emit("PUSH_INT", node[1])
        elif op == "string":
            self.emit("PUSH_STR", node[1])
        elif op == "bool":
            self.emit("PUSH_BOOL", 1 if node[1] else 0)
        elif op == "ident":
            self.emit("LOAD", node[1])
        elif op == "list":
            elements = node[1]
            for elem in elements:
                self.compile_expr(elem)
            self.emit("BUILD_LIST", len(elements))
        elif op == "index":
            self.compile_expr(node[1])
            self.compile_expr(node[2])
            self.emit("INDEX")
        elif op == "slice":
            self.compile_expr(node[1])
            self.compile_expr(node[2])
            self.compile_expr(node[3])
            self.emit("SLICE")
        elif op == "func_call":
            func_node, args = node[1], node[2]
            for arg in args:
                self.compile_expr(arg)
            if func_node[0] != "ident":
                raise NotImplementedError("Only direct function calls are supported")
            self.emit("CALL", func_node[1])
        elif op == "unary":
            unary_op = node[1]
            self.compile_expr(node[2])
            if unary_op == Op.SUB:
                self.emit("NEG")
            elif unary_op == Op.NOT_BITS:
                self.emit("NOT")
        elif isinstance(op, Op):
            self.compile_expr(node[1])
            self.compile_expr(node[2])
            op_map = {
                Op.ADD: "ADD",
                Op.SUB: "SUB",
                Op.MUL: "MUL",
                Op.DIV: "DIV",
                Op.MOD: "MOD",
                Op.EQ: "EQ",
                Op.NE: "NE",
                Op.GT: "GT",
                Op.LT: "LT",
                Op.GE: "GE",
                Op.LE: "LE",
                Op.AND: "AND",
                Op.OR: "OR",
            }
            self.emit(op_map[op])
        else:
            raise NotImplementedError(f"Unsupported expression node: {node}")


def compile_source(source: str, file: str = "<stdin>") -> str:
    """Compile OMG source string to bytecode."""
    tokens, token_map = tokenize(source)
    parser = Parser(tokens, token_map, file)
    ast = parser.parse()
    compiler = Compiler()
    return compiler.compile(ast)


def main(argv: List[str]) -> int:
    import sys

    if not argv:
        print("Usage: python -m omglang.bytecode <script.omg>")
        return 1
    path = argv[0]
    with open(path, "r", encoding="utf-8") as f:
        src = f.read()
    bc = compile_source(src, path)
    sys.stdout.write(bc)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    import sys

    raise SystemExit(main(sys.argv[1:]))
