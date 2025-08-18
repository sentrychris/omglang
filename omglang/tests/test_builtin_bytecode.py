"""Tests for builtin function compilation to bytecode."""

import pytest

from omglang.compiler import compile_source, disassemble


@pytest.mark.parametrize(
    "src, expected",
    [
        ("emit length([1])", "BUILTIN length 1"),
        ("emit chr(65)", "BUILTIN chr 1"),
        ('emit ascii("A")', "BUILTIN ascii 1"),
        ("emit hex(255)", "BUILTIN hex 1"),
        ("emit binary(5)", "BUILTIN binary 1"),
        ("emit binary(5, 3)", "BUILTIN binary 2"),
        ('emit read_file("README.MD")', "BUILTIN read_file 1"),
        ('emit write_file("out.omgb", "data")', "BUILTIN write_file 2"),
        ('emit file_open("README.MD", "r")', "BUILTIN file_open 2"),
        ("emit file_read(0)", "BUILTIN file_read 1"),
        ('emit file_write(0, "x")', "BUILTIN file_write 2"),
        ("emit file_close(0)", "BUILTIN file_close 1"),
        ('emit file_exists("README.MD")', "BUILTIN file_exists 1"),
    ],
)
def test_builtin_emits_builtin_instruction(src: str, expected: str) -> None:
    """Ensure calls to builtins compile to BUILTIN instructions."""
    bc = compile_source(src)
    lines = disassemble(bc).splitlines()
    assert expected in lines
    name = expected.split()[1]
    assert f"CALL {name}" not in lines
