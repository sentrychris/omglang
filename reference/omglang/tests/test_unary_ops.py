"""
Tests for unary operations in OMG Language
"""
from omglang.operations import Op
from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_unary_ops_parse_and_runtime(capsys):
    """
    Test that unary operations are parsed correctly and execute as expected.
    """
    source = (
        "emit -5\n"
        "emit +5\n"
        "alloc a := 2\n"
        "emit -a\n"
        "emit +a\n"
    )
    ast = parse_source(source)

    emit_neg = ast[0]
    emit_pos = ast[1]

    assert emit_neg[1][0] == 'unary'
    assert emit_neg[1][1] == Op.SUB
    assert emit_pos[1][0] == 'unary'
    assert emit_pos[1][1] == Op.ADD

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['-5', '5', '-2', '2']
