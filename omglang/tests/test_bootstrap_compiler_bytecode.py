"""Ensure bootstrap compiler matches Python serializer."""
from pathlib import Path
import subprocess
import sys

from omglang.compiler import compile_source


def test_bootstrap_compiler_matches_python(tmp_path):
    src = 'emit 1 + 2'
    src_path = tmp_path / 'prog.omg'
    src_path.write_text(src, encoding='utf-8')
    out_path = tmp_path / 'prog.omgb'

    root = Path(__file__).resolve().parents[2]
    compiler = root / 'bootstrap' / 'compiler.omg'
    runner = root / 'omg.py'
    subprocess.check_call([sys.executable, str(runner), str(compiler), str(src_path), str(out_path)])

    data_boot = out_path.read_bytes()
    data_py = compile_source(src, str(src_path))
    assert data_boot == data_py
