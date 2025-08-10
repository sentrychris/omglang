"""OMG bytecode compiler.
This module compiles OMG source code into a textual bytecode format consumed by the
native Rust virtual machine.  It relies on the existing lexer and parser to produce
an AST and then lowers that tree into a simple stack-based instruction set.
Usage:
    python -m omglang.compiler path/to/script.omg [output.omgb]

File: compiler.py
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © 2025 Chris Rowles. All rights reserved.
Version: 0.1.1
License: MIT
"""

from __future__ import annotations
import sys
import struct

from dataclasses import dataclass
from typing import List, Tuple

from omglang.lexer import tokenize
from omglang.parser import Parser
from omglang.operations import Op


Instr = Tuple[str, object | None]

# Mapping of instruction mnemonics to opcode numbers.
OPCODES: dict[str, int] = {
    "PUSH_INT": 0,
    "PUSH_STR": 1,
    "PUSH_BOOL": 2,
    "BUILD_LIST": 3,
    "BUILD_DICT": 4,
    "LOAD": 5,
    "STORE": 6,
    "ADD": 7,
    "SUB": 8,
    "MUL": 9,
    "DIV": 10,
    "MOD": 11,
    "EQ": 12,
    "NE": 13,
    "LT": 14,
    "LE": 15,
    "GT": 16,
    "GE": 17,
    "BAND": 18,
    "BOR": 19,
    "BXOR": 20,
    "SHL": 21,
    "SHR": 22,
    "AND": 23,
    "OR": 24,
    "NOT": 25,
    "NEG": 26,
    "INDEX": 27,
    "SLICE": 28,
    "JUMP": 29,
    "JUMP_IF_FALSE": 30,
    "CALL": 31,
    "TCALL": 32,
    "BUILTIN": 33,
    "POP": 34,
    "PUSH_NONE": 35,
    "RET": 36,
    "EMIT": 37,
    "HALT": 38,
    "STORE_INDEX": 39,
    "ATTR": 40,
    "STORE_ATTR": 41,
    "ASSERT": 42,
    "CALL_VALUE": 43,
    "SETUP_EXCEPT": 44,
    "POP_BLOCK": 45,
    "RAISE": 46,
    "RAISE_SYNTAX_ERROR": 47,
    "RAISE_TYPE_ERROR": 48,
    "RAISE_UNDEF_IDENT_ERROR": 49,
    "RAISE_VALUE_ERROR": 50,
    "RAISE_MODULE_IMPORT_ERROR": 51,
}

# Reverse-mapped opcode mnemonics
REV_OPCODES: dict[int, str] = {v: k for k, v in OPCODES.items()}

# Bytecode header
MAGIC_HEADER = b"OMGB"

# Encoded as 0x00MMmmpp where MM=major, mm=minor, pp=patch
BC_VERSION = (0 << 16) | (1 << 8) | 1


@dataclass
class FunctionEntry:
    """Metadata for a compiled function."""
    name: str
    params: List[str]
    address: int


class Compiler:
    """Compile OMG AST nodes into bytecode instructions."""

    def __init__(self) -> None:
        """
        Initialize the compiler state.
        """
        self.code: List[Instr] = []
        self.pending_funcs: List[Tuple[str, List[str], List[Instr]]] = []
        self.funcs: List[FunctionEntry] = []
        # Track outstanding `break` statements within loops.  Each entry on
        # the stack holds placeholder jump indices that should be patched to
        # the end of the current loop.
        self.break_stack: List[List[int]] = []
        self.builtins = {
            "chr",
            "ascii",
            "hex",
            "binary",
            "length",
            "read_file",
            "freeze",
            "call_builtin",
        }

    # ------------------------------------------------------------------
    # Helper utilities
    # ------------------------------------------------------------------
    def emit(self, op: str, arg: object | None = None) -> None:
        """
        Emit a bytecode instruction.
        """
        self.code.append((op, arg))

    def emit_placeholder(self, op: str) -> int:
        """
        Emit a placeholder bytecode instruction.
        """
        idx = len(self.code)
        self.code.append((op, None))
        return idx

    def patch(self, idx: int, target: int) -> None:
        """
        Patch a placeholder instruction with a target address.
        """
        op, _ = self.code[idx]
        self.code[idx] = (op, target)

    # ------------------------------------------------------------------
    # Compilation entry points
    # ------------------------------------------------------------------
    def compile(self, ast: List[tuple]) -> bytes:
        """
        Compile the given AST into binary bytecode.
        """
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

        out = bytearray(MAGIC_HEADER)
        out.extend(struct.pack("<I", BC_VERSION))
        out.extend(struct.pack("<I", len(self.funcs)))
        for f in self.funcs:
            name_bytes = f.name.encode("utf-8")
            out.extend(struct.pack("<I", len(name_bytes)))
            out.extend(name_bytes)
            out.extend(struct.pack("<I", len(f.params)))
            for p in f.params:
                pb = p.encode("utf-8")
                out.extend(struct.pack("<I", len(pb)))
                out.extend(pb)
            out.extend(struct.pack("<I", f.address))

        out.extend(struct.pack("<I", len(final_code)))
        for op, arg in final_code:
            out.append(OPCODES[op])
            if op == "PUSH_INT" and isinstance(arg, int):
                out.extend(struct.pack("<q", arg))
            elif op == "PUSH_STR" and isinstance(arg, str):
                sb = arg.encode("utf-8")
                out.extend(struct.pack("<I", len(sb)))
                out.extend(sb)
            elif op == "PUSH_BOOL" and isinstance(arg, bool):
                out.append(1 if arg else 0)
            elif op in {"BUILD_LIST", "BUILD_DICT", "CALL_VALUE"} and isinstance(arg, int):
                out.extend(struct.pack("<I", arg))
            elif op in {"LOAD", "STORE", "CALL", "TCALL", "ATTR", "STORE_ATTR"} and isinstance(arg, str):
                sb = arg.encode("utf-8")
                out.extend(struct.pack("<I", len(sb)))
                out.extend(sb)
            elif op == "BUILTIN" and isinstance(arg, tuple):
                name, argc = arg
                sb = name.encode("utf-8")
                out.extend(struct.pack("<I", len(sb)))
                out.extend(sb)
                out.extend(struct.pack("<I", argc))
            elif op in {"JUMP", "JUMP_IF_FALSE", "SETUP_EXCEPT"} and isinstance(arg, int):
                out.extend(struct.pack("<I", arg))
            # Remaining instructions carry no operands.

        return bytes(out)

    # ------------------------------------------------------------------
    # Statement compilation
    # ------------------------------------------------------------------
    def compile_block(self, block: List[tuple]) -> None:
        """
        Compile a block of statements.
        """
        for stmt in block:
            self.compile_stmt(stmt)

    def compile_stmt(self, stmt: tuple) -> None:
        """
        Compile a single statement node.
        """
        kind = stmt[0]
        if kind == "emit":
            self.compile_expr(stmt[1])
            self.emit("EMIT")
        elif kind == "decl" or kind == "assign":
            name, expr = stmt[1], stmt[2]
            self.compile_expr(expr)
            self.emit("STORE", name)
        elif kind == "attr_assign":
            base, attr, expr = stmt[1], stmt[2], stmt[3]
            self.compile_expr(base)
            self.compile_expr(expr)
            self.emit("STORE_ATTR", attr)
        elif kind == "index_assign":
            base, index_expr, value_expr = stmt[1], stmt[2], stmt[3]
            self.compile_expr(base)
            self.compile_expr(index_expr)
            self.compile_expr(value_expr)
            self.emit("STORE_INDEX")
        elif kind == "expr_stmt":
            self.compile_expr(stmt[1])
            self.emit("POP")
        elif kind == "import":
            raise NotImplementedError(
                "Module imports are resolved by the interpreter and cannot be compiled",
            )
        elif kind == "facts":
            self.compile_expr(stmt[1])
            self.emit("ASSERT")
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
            self.break_stack.append([])
            self.compile_block(body)
            self.emit("JUMP", start)
            self.patch(jf, len(self.code))
            # Patch any breaks that occurred inside the loop
            for idx in self.break_stack.pop():
                self.patch(idx, len(self.code))
        elif kind == "try":
            try_block, exc_name, except_block = stmt[1], stmt[2], stmt[3]
            handler_idx = self.emit_placeholder("SETUP_EXCEPT")
            try_body = try_block[1] if try_block[0] == "block" else []
            self.compile_block(try_body)
            self.emit("POP_BLOCK")
            end_jump = self.emit_placeholder("JUMP")
            self.patch(handler_idx, len(self.code))
            if exc_name:
                self.emit("STORE", exc_name)
            except_body = except_block[1] if except_block[0] == "block" else []
            self.compile_block(except_body)
            self.patch(end_jump, len(self.code))
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
                name = func_node[1]
                for arg in args:
                    self.compile_expr(arg)
                if name in self.builtins:
                    self.emit("BUILTIN", (name, len(args)))
                    self.emit("RET")
                else:
                    self.emit("TCALL", name)
            else:
                self.compile_expr(expr)
                self.emit("RET")
        elif kind == "break":
            if not self.break_stack:
                raise SyntaxError("'break' used outside of loop")
            j = self.emit_placeholder("JUMP")
            self.break_stack[-1].append(j)
        elif kind == "block":
            self.compile_block(stmt[1])
        else:
            raise NotImplementedError(f"Unsupported statement: {stmt}")

    def _compile_function_body(self, body: List[tuple]) -> List[Instr]:
        """
        Compile the body of a function into bytecode.
        This is used to handle function definitions separately.
        """
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
        """
        Compile an expression node into bytecode.
        """
        op = node[0]
        if op == "number":
            self.emit("PUSH_INT", node[1])
        elif op == "string":
            self.emit("PUSH_STR", node[1])
        elif op == "bool":
            self.emit("PUSH_BOOL", bool(node[1]))
        elif op == "ident":
            self.emit("LOAD", node[1])
        elif op == "list":
            elements = node[1]
            for elem in elements:
                self.compile_expr(elem)
            self.emit("BUILD_LIST", len(elements))
        elif op == "dict":
            pairs = node[1]
            for key, val in pairs:
                self.emit("PUSH_STR", key)
                self.compile_expr(val)
            self.emit("BUILD_DICT", len(pairs))
        elif op == "index":
            self.compile_expr(node[1])
            self.compile_expr(node[2])
            self.emit("INDEX")
        elif op == "slice":
            self.compile_expr(node[1])
            self.compile_expr(node[2])
            end = node[3]
            if end is None:
                self.emit("PUSH_NONE")
            else:
                self.compile_expr(end)
            self.emit("SLICE")
        elif op == "dot":
            self.compile_expr(node[1])
            self.emit("ATTR", node[2])
        elif op == "func_call":
            func_node, args = node[1], node[2]
            if func_node[0] == "ident":
                name = func_node[1]
                if name in {"panic", "raise"}:
                    if args:
                        self.compile_expr(args[0])
                    else:
                        self.emit("PUSH_STR", "")
                    self.emit("RAISE")
                elif name == "_omg_vm_syntax_error_handle":
                    if args:
                        self.compile_expr(args[0])
                    else:
                        self.emit("PUSH_STR", "")
                    self.emit("RAISE_SYNTAX_ERROR")
                elif name == "_omg_vm_type_error_handle":
                    if args:
                        self.compile_expr(args[0])
                    else:
                        self.emit("PUSH_STR", "")
                    self.emit("RAISE_TYPE_ERROR")
                elif name == "_omg_vm_undef_ident_error_handle":
                    if args:
                        self.compile_expr(args[0])
                    else:
                        self.emit("PUSH_STR", "")
                    self.emit("RAISE_UNDEF_IDENT_ERROR")
                elif name == "_omg_vm_value_error_handle":
                    if args:
                        self.compile_expr(args[0])
                    else:
                        self.emit("PUSH_STR", "")
                    self.emit("RAISE_VALUE_ERROR")
                elif name == "_omg_vm_module_import_error_handle":
                    if args:
                        self.compile_expr(args[0])
                    else:
                        self.emit("PUSH_STR", "")
                    self.emit("RAISE_MODULE_IMPORT_ERROR")
                else:
                    for arg in args:
                        self.compile_expr(arg)
                    if name in self.builtins:
                        self.emit("BUILTIN", (name, len(args)))
                    else:
                        self.emit("CALL", name)
            else:
                # General case: evaluate function expression then call indirectly
                self.compile_expr(func_node)
                for arg in args:
                    self.compile_expr(arg)
                self.emit("CALL_VALUE", len(args))
        elif op == "unary":
            unary_op = node[1]
            self.compile_expr(node[2])
            if unary_op == Op.SUB:
                self.emit("NEG")
            elif unary_op == Op.NOT_BITS:
                self.emit("NOT")
            elif unary_op == Op.ADD:
                pass
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
                Op.AND_BITS: "BAND",
                Op.OR_BITS: "BOR",
                Op.XOR_BITS: "BXOR",
                Op.SHL: "SHL",
                Op.SHR: "SHR",
            }
            self.emit(op_map[op])
        else:
            raise NotImplementedError(f"Unsupported expression node: {node}")


def compile_source(source: str, file: str = "<stdin>") -> bytes:
    """
    Compile OMG source string to binary bytecode.
    """
    tokens, token_map = tokenize(source)
    parser = Parser(tokens, token_map, file)
    ast = parser.parse()
    compiler = Compiler()
    return compiler.compile(ast)


def disassemble(data: bytes) -> str:
    """Convert binary bytecode back to a textual representation."""
    idx = 0
    if data[:4] != MAGIC_HEADER:
        raise ValueError("Invalid bytecode header")
    idx = 4
    version = struct.unpack_from("<I", data, idx)[0]
    if version != BC_VERSION:
        raise ValueError(f"unsupported version: {version}")
    idx += 4
    func_count = struct.unpack_from("<I", data, idx)[0]
    idx += 4
    lines: List[str] = []
    for _ in range(func_count):
        name_len = struct.unpack_from("<I", data, idx)[0]
        idx += 4
        name = data[idx:idx + name_len].decode("utf-8")
        idx += name_len
        param_count = struct.unpack_from("<I", data, idx)[0]
        idx += 4
        params = []
        for _ in range(param_count):
            p_len = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            param = data[idx:idx + p_len].decode("utf-8")
            idx += p_len
            params.append(param)
        addr = struct.unpack_from("<I", data, idx)[0]
        idx += 4
        lines.append(
            f"FUNC {name} {param_count} {' '.join(params)} {addr}"
        )
    code_len = struct.unpack_from("<I", data, idx)[0]
    idx += 4
    for _ in range(code_len):
        op = data[idx]
        idx += 1
        name = REV_OPCODES[op]
        if name == "PUSH_INT":
            (v,) = struct.unpack_from("<q", data, idx)
            idx += 8
            lines.append(f"PUSH_INT {v}")
        elif name == "PUSH_STR":
            slen = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            s = data[idx:idx + slen].decode("utf-8")
            idx += slen
            lines.append(f"PUSH_STR {s}")
        elif name == "PUSH_BOOL":
            b = data[idx] != 0
            idx += 1
            lines.append(f"PUSH_BOOL {int(b)}")
        elif name in {"BUILD_LIST", "BUILD_DICT", "CALL_VALUE"}:
            (n,) = struct.unpack_from("<I", data, idx)
            idx += 4
            lines.append(f"{name} {n}")
        elif name in {"LOAD", "STORE", "CALL", "TCALL", "ATTR", "STORE_ATTR"}:
            slen = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            s = data[idx:idx + slen].decode("utf-8")
            idx += slen
            lines.append(f"{name} {s}")
        elif name == "BUILTIN":
            slen = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            s = data[idx:idx + slen].decode("utf-8")
            idx += slen
            argc = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            lines.append(f"BUILTIN {s} {argc}")
        elif name in {"JUMP", "JUMP_IF_FALSE", "SETUP_EXCEPT"}:
            (t,) = struct.unpack_from("<I", data, idx)
            idx += 4
            lines.append(f"{name} {t}")
        else:
            lines.append(name)
    return "\n".join(lines)


def main(argv: List[str]) -> int:
    """
    Entry point for the CLI.
    """
    if not argv:
        print("Usage: python -m omglang.compiler <script.omg> [output.omgb]")
        return 1
    path = argv[0]
    with open(path, "r", encoding="utf-8") as f:
        src = f.read()
    bc = compile_source(src, path)
    if len(argv) > 1:
        out_path = argv[1]
        with open(out_path, "wb") as f:
            f.write(bc)
    else:
        sys.stdout.buffer.write(bc)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main(sys.argv[1:]))
