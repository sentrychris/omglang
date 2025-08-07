"""Tests for bytecode CLI output encoding."""
import subprocess
import sys


def test_bytecode_cli_writes_utf8(tmp_path):
    """Ensure bytecode written via CLI is UTF-8 encoded."""
    src = 'emit chr(65)'
    src_path = tmp_path / "t.omg"
    src_path.write_text(src, encoding="utf-8")
    out_path = tmp_path / "t.bc"
    subprocess.check_call([sys.executable, "-m", "omglang.bytecode", str(src_path), str(out_path)])
    data = out_path.read_bytes()
    text = data.decode("utf-8")
    assert "BUILTIN chr 1" in text
