"""Tests for bytecode CLI output encoding."""
import struct
import subprocess
import sys


def test_bytecode_cli_writes_utf8(tmp_path):
    """Ensure bytecode written via CLI is UTF-8 encoded."""
    src = 'emit chr(65)'
    src_path = tmp_path / "t.omg"
    src_path.write_text(src, encoding="utf-8")
    out_path = tmp_path / "t.omgb"
    subprocess.check_call([sys.executable, "-m", "omglang.compiler", str(src_path), str(out_path)])
    data = out_path.read_bytes()
    # Compiled bytecode should start with the magic header
    assert data[:4] == b"OMGB"
    from omglang.compiler import BC_VERSION, disassemble
    assert struct.unpack_from("<I", data, 4)[0] == BC_VERSION
    text = disassemble(data)
    assert "BUILTIN chr 1" in text
