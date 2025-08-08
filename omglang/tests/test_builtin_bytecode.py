"""Tests for builtin function compilation to bytecode."""
import pytest

from omglang.compiler import compile_source, disassemble


@pytest.mark.parametrize(
    "src, expected",
    [
        ("emit length([1])", "BUILTIN length 1"),
        ("emit chr(65)", "BUILTIN chr 1"),
        ("emit ascii(\"A\")", "BUILTIN ascii 1"),
        ("emit hex(255)", "BUILTIN hex 1"),
        ("emit binary(5)", "BUILTIN binary 1"),
        ("emit binary(5, 3)", "BUILTIN binary 2"),
        ("emit read_file(\"README.MD\")", "BUILTIN read_file 1"),
    ],
)
def test_builtin_emits_builtin_instruction(src: str, expected: str) -> None:
    """Ensure calls to builtins compile to BUILTIN instructions."""
    bc = compile_source(src)
    lines = disassemble(bc).splitlines()
    assert expected in lines
    name = expected.split()[1]
    assert f"CALL {name}" not in lines
