"""Tests for file I/O builtins."""

from omglang.interpreter import Interpreter
from omglang.tests.utils import parse_source


def test_file_io_text_and_exists(tmp_path, capsys):
    file = tmp_path / "out.txt"
    fp = str(file).replace("\\", "\\\\")
    source = (
        f'alloc h := file_open("{fp}", "w")\n'
        f'file_write(h, "hi")\n'
        f'file_close(h)\n'
        f'alloc h := file_open("{fp}", "r")\n'
        f'emit file_read(h)\n'
        f'file_close(h)\n'
        f'emit file_exists("{fp}")\n'
        f'emit file_exists("{fp}missing")\n'
    )
    ast = parse_source(source)
    interpreter = Interpreter("<test>")
    interpreter.execute(ast)
    out = capsys.readouterr().out.strip().splitlines()
    assert out[0] == "hi"
    assert out[1:] == ["True", "False"]


def test_file_io_binary(tmp_path, capsys):
    file = tmp_path / "data.bin"
    fp = str(file).replace("\\", "\\\\")
    source = (
        f'alloc h := file_open("{fp}", "wb")\n'
        f'file_write(h, [1,2,3])\n'
        f'file_close(h)\n'
        f'alloc h := file_open("{fp}", "rb")\n'
        f'emit file_read(h)\n'
        f'file_close(h)\n'
    )
    ast = parse_source(source)
    interpreter = Interpreter("<test>")
    interpreter.execute(ast)
    out = capsys.readouterr().out.strip().splitlines()
    assert out == ["[1, 2, 3]"]
