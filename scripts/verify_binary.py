"""verify_binary
=================

Perform basic sanity checks on a compiled OMG interpreter binary.

performs a two-pass decode/validate process and additional semantic checks:

* Decode the entire function table and instruction stream
* Ensure function entry point addresses and jump targets point to valid
  instruction indices.
* Confirm that `CALL` and `TCALL` instructions reference functions that
  exist in the function table.
* Verify that the final instruction is `RET` and that the file contains no
  trailing bytes.
"""

import os
import struct
import sys

from omglang.compiler import MAGIC_HEADER, REV_OPCODES


def _read_u32(data: bytes, idx: int) -> tuple[int, int]:
    """Return a little-endian unsigned 32-bit int and the new index."""

    return struct.unpack_from("<I", data, idx)[0], idx + 4


def _read_str(data: bytes, idx: int) -> tuple[str, int]:
    """Return a length-prefixed UTF‑8 string and the new index."""

    slen, idx = _read_u32(data, idx)
    return data[idx:idx + slen].decode("utf-8"), idx + slen

def verify_interpreter(interp_bin: str) -> None:
    """Decode and validate ``interpreter.omgb``.

    The function raises ``ValueError`` if any structural check fails.  On
    success, a few details about the binary are printed.
    """

    with open(interp_bin, "rb") as f:
        data = f.read()
    print("len", len(data))

    # ------------------------------------------------------------------
    # Decode pass
    # ------------------------------------------------------------------
    if data[:4] != MAGIC_HEADER:
        raise ValueError(f"Bad magic: {data[:4]!r}")
    print(f"4-byte header {data[:4]!r} is valid")
    idx = 4

    func_count, idx = _read_u32(data, idx)
    print("func_count", func_count)
    functions: dict[str, int] = {}
    for _ in range(func_count):
        name, idx = _read_str(data, idx)
        param_count, idx = _read_u32(data, idx)
        for _ in range(param_count):
            _, idx = _read_str(data, idx)
        addr, idx = _read_u32(data, idx)
        functions[name] = addr
        print("func", name, param_count, addr)

    code_len, idx = _read_u32(data, idx)
    print("code_len", code_len)
    instructions: list[dict[str, object]] = []
    for ins in range(code_len):
        if idx >= len(data):
            raise ValueError(f"unexpected end at instruction {ins}")
        op = data[idx]
        idx += 1
        name = REV_OPCODES.get(op)
        if name is None:
            raise ValueError(f"unknown opcode {op} at {ins}")
        arg: object | None = None
        if name == "PUSH_INT":
            if idx + 8 > len(data):
                raise ValueError("int beyond end of file")
            arg = struct.unpack_from("<q", data, idx)[0]
            idx += 8
        elif name == "PUSH_STR":
            arg, idx = _read_str(data, idx)
        elif name == "PUSH_BOOL":
            if idx >= len(data):
                raise ValueError("bool beyond end of file")
            arg = bool(data[idx])
            idx += 1
        elif name in {"BUILD_LIST", "BUILD_DICT", "CALL_VALUE", "JUMP", "JUMP_IF_FALSE"}:
            arg, idx = _read_u32(data, idx)
        elif name == "BUILTIN":
            bname, idx = _read_str(data, idx)
            argc, idx = _read_u32(data, idx)
            arg = (bname, argc)
        elif name in {"LOAD", "STORE", "CALL", "TCALL", "ATTR", "STORE_ATTR"}:
            arg, idx = _read_str(data, idx)
        # other opcodes carry no immediate argument
        instructions.append({"idx": ins, "name": name, "arg": arg})

    if idx != len(data):
        raise ValueError(f"extra {len(data) - idx} bytes at end of file")

    # ------------------------------------------------------------------
    # Validation pass
    # ------------------------------------------------------------------
    for fname, addr in functions.items():
        if not 0 <= addr < code_len:
            raise ValueError(f"function {fname!r} has invalid address {addr}")

    for inst in instructions:
        name = inst["name"]
        arg = inst["arg"]
        if name in {"JUMP", "JUMP_IF_FALSE"}:
            if not 0 <= int(arg) < code_len:
                raise ValueError(
                    f"{name} at {inst['idx']} targets invalid address {arg}"
                )
        elif name in {"CALL", "TCALL"}:
            if str(arg) not in functions:
                raise ValueError(
                    f"{name} at {inst['idx']} references unknown function {arg!r}"
                )

    if instructions and instructions[-1]["name"] != "RET":
        print("warning: last instruction is not RET")

    print(f"Verified {len(functions)} functions and {len(instructions)} instructions")
    print("[✓] Binary is verified")


if __name__ == "__main__":
    verify_interpreter(
        os.path.join(
            os.path.dirname(os.path.dirname(__file__)), "runtime", "interpreter.omgb"
        )
        if len(sys.argv) == 1
        else sys.argv[1]
    )
