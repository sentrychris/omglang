"""
Tests for the 'or' conditionals in OMG Language
"""
from omglang.operations import Op
from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_if_or_ast_and_runtime(capsys):
    """
    Test that the AST for an `if` statement with an `or` condition is correct.
    """
    source = (
        "alloc a := false\n"
        "alloc b := true\n"
        "if a or b { emit 1 }\n"
    )
    ast = parse_source(source)
    if_stmt = ast[2]
    cond = if_stmt[1]
    assert cond[0] == Op.OR
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['1']


def test_elif_or_runtime(capsys):
    """
    Test that an `elif` with an `or` condition executes correctly.
    """
    source = (
        "alloc a := false\n"
        "alloc b := false\n"
        "alloc c := true\n"
        "if a or b { emit 1 } elif b or c { emit 2 }\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['2']


def test_comparison_or_precedence(capsys):
    """
    Test that comparisons with `or` have the correct precedence.
    """
    source = (
        "alloc a := 1\n"
        "alloc b := 2\n"
        "if a == 1 or b == 2 { emit 1 }\n"
    )
    ast = parse_source(source)
    cond = ast[2][1]
    assert cond[0] == Op.OR
    assert cond[1][0] == Op.EQ
    assert cond[2][0] == Op.EQ
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['1']


def test_or_and_precedence(capsys):
    """
    Test that `or` has lower precedence than `and`.
    """
    source = (
        "alloc a := false\n"
        "alloc b := true\n"
        "alloc c := true\n"
        "if a and b or c { emit 1 }\n"
    )
    ast = parse_source(source)
    cond = ast[3][1]
    assert cond[0] == Op.OR
    assert cond[1][0] == Op.AND
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['1']


def test_or_short_circuits(capsys):
    """
    Test that `or` short-circuits evaluation.
    """
    source = (
        "proc rhs() {\n"
        "    emit \"rhs\"\n"
        "    return true\n"
        "}\n"
        "if true or rhs() { emit \"ok\" } else { emit \"bad\" }\n"
        "if true or (1 / 0) { emit \"ok\" } else { emit \"bad\" }\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['ok', 'ok']
