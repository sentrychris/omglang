"""Verify a compiled OMG interpreter binary.

The script performs a thorough sanity check of an ``interpreter.omgb`` file
emitted by :mod:`omglang.compiler`.  It walks the header and function table
before decoding the instruction stream, validating that each operand fits
within the binary and references valid data.

Checks performed include:

* Magic header and overall layout
* Function names/parameter names decode as UTF-8
* Function entry addresses point into the code section
* All opcodes are known and have fully formed operands
* Jump targets are within the instruction range
* ``CALL``/``TCALL`` instructions reference existing functions
* Booleans are encoded as ``0`` or ``1``

Any failure raises a :class:`ValueError` describing the issue.
"""

from __future__ import annotations

import os
import struct
import sys
from typing import Dict, List, Tuple

from omglang.compiler import MAGIC_HEADER, REV_OPCODES


Instr = Tuple[str, object | None]


def _read_str(data: bytes, idx: int) -> Tuple[str, int]:
    """Read a length-prefixed UTF-8 string starting at ``idx``."""

    if idx + 4 > len(data):
        raise ValueError("Unexpected end while reading string length")
    slen = struct.unpack_from("<I", data, idx)[0]
    idx += 4
    end = idx + slen
    if end > len(data):
        raise ValueError("String extends beyond end of file")
    try:
        value = data[idx:end].decode("utf-8")
    except UnicodeDecodeError as exc:  # pragma: no cover - just in case
        raise ValueError("String failed to decode as UTF-8") from exc
    return value, end


def verify_interpreter(interp_bin: str) -> None:
    """Verify the interpreter binary contains full opcode instructions."""

    with open(interp_bin, "rb") as file:
        data = file.read()
    print("len", len(data))

    # ------------------------------------------------------------------
    # Header and function table
    # ------------------------------------------------------------------
    if data[:4] != MAGIC_HEADER:
        raise ValueError(f"Bad magic: {data[:4]!r}")
    print(f"4-byte header {data[:4]!r} is valid")

    idx = 4
    func_count = struct.unpack_from("<I", data, idx)[0]
    idx += 4
    print("func_count", func_count)

    functions: Dict[str, int] = {}
    for _ in range(func_count):
        name, idx = _read_str(data, idx)
        param_total = struct.unpack_from("<I", data, idx)[0]
        idx += 4
        params: List[str] = []
        for _ in range(param_total):
            param, idx = _read_str(data, idx)
            params.append(param)
        addr = struct.unpack_from("<I", data, idx)[0]
        idx += 4
        if name in functions:
            raise ValueError(f"Duplicate function name {name!r}")
        functions[name] = addr
        print("func", name, params, addr)

    # ------------------------------------------------------------------
    # Bytecode stream
    # ------------------------------------------------------------------
    code_len = struct.unpack_from("<I", data, idx)[0]
    idx += 4
    print("code_len", code_len)

    instructions: List[Instr] = []
    for ins in range(code_len):
        if idx >= len(data):
            raise ValueError(
                f"Unexpected end at instruction {ins}, idx {idx}, len {len(data)}"
            )
        op = data[idx]
        idx += 1
        name = REV_OPCODES.get(op)
        if name is None:
            raise ValueError(f"Unknown opcode {op} at instruction {ins}")

        arg: object | None = None
        if name == "PUSH_INT":
            end = idx + 8
            if end > len(data):
                raise ValueError("PUSH_INT operand beyond end")
            arg = struct.unpack_from("<q", data, idx)[0]
            idx = end
        elif name == "PUSH_STR":
            arg, idx = _read_str(data, idx)
        elif name == "PUSH_BOOL":
            if idx >= len(data):
                raise ValueError("PUSH_BOOL operand missing")
            val = data[idx]
            idx += 1
            if val not in (0, 1):
                raise ValueError(f"Invalid bool value {val}")
            arg = bool(val)
        elif name in {"BUILD_LIST", "BUILD_DICT", "CALL_VALUE"}:
            end = idx + 4
            if end > len(data):
                raise ValueError(f"{name} operand beyond end")
            arg = struct.unpack_from("<I", data, idx)[0]
            idx = end
        elif name in {"JUMP", "JUMP_IF_FALSE"}:
            end = idx + 4
            if end > len(data):
                raise ValueError(f"{name} operand beyond end")
            target = struct.unpack_from("<I", data, idx)[0]
            idx = end
            if target >= code_len:
                raise ValueError(f"{name} target {target} out of range")
            arg = target
        elif name in {"LOAD", "STORE", "CALL", "TCALL", "ATTR", "STORE_ATTR"}:
            arg, idx = _read_str(data, idx)
            if name in {"CALL", "TCALL"} and arg not in functions:
                raise ValueError(f"{name} references unknown function {arg!r}")
        elif name == "BUILTIN":
            bname, idx = _read_str(data, idx)
            if idx + 4 > len(data):
                raise ValueError("BUILTIN argc beyond end")
            argc = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            arg = (bname, argc)
        # Remaining instructions carry no operands.

        instructions.append((name, arg))

    if len(instructions) != code_len:
        raise ValueError(
            f"Instruction count mismatch: expected {code_len}, got {len(instructions)}"
        )

    if idx != len(data):
        raise ValueError(f"Trailing {len(data) - idx} bytes at end of file")

    for fname, addr in functions.items():
        if addr >= code_len:
            raise ValueError(f"Function {fname!r} address {addr} out of range")

    print(
        f"[✓] Verified {len(functions)} functions and {len(instructions)} instructions"
    )


if __name__ == "__main__":
    verify_interpreter(
        os.path.join(
            os.path.dirname(os.path.dirname(__file__)),
            "runtime",
            "interpreter.omgb",
        )
        if len(sys.argv) == 1
        else sys.argv[1]
    )
