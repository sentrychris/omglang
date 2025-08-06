"""
Tests for `if` statements with `and` conditions in OMG Language.
"""
import os
import sys

from omglang.lexer import tokenize, Token
from omglang.parser import Parser
from omglang.operations import Op
from omglang.interpreter import Interpreter

sys.path.append(os.path.dirname(os.path.dirname(__file__)))


def parse_source(source: str):
    """
    Parse the source code and return the AST.
    """
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token('EOF', None, eof_line))
    parser = Parser(tokens, token_map, '<test>')
    return parser.parse()


def test_if_and_ast_and_runtime(capsys):
    """
    Test that the AST for an `if` statement with an `and` condition is correct
    """
    source = (
        "alloc a := true\n"
        "alloc b := true\n"
        "if a and b { emit 1 }\n"
    )
    ast = parse_source(source)
    if_stmt = ast[2]
    cond = if_stmt[1]
    assert cond[0] == Op.AND
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['1']


def test_elif_and_runtime(capsys):
    """
    Test that an `elif` with an `and` condition executes correctly.
    """
    source = (
        "alloc a := true\n"
        "alloc b := false\n"
        "alloc c := true\n"
        "if a and b { emit 1 } elif a and c { emit 2 }\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['2']


def test_comparison_and_precedence(capsys):
    """
    Test that `and` has lower precedence than comparison operators."""
    source = (
        "alloc a := 1\n"
        "alloc b := 2\n"
        "if a == 1 and b == 2 { emit 1 }\n"
    )
    ast = parse_source(source)
    cond = ast[2][1]
    assert cond[0] == Op.AND
    assert cond[1][0] == Op.EQ
    assert cond[2][0] == Op.EQ
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['1']


def test_and_short_circuits(capsys):
    """
    Test that `and` short-circuits and does not evaluate the right-hand side
    """
    source = (
        "proc rhs() {\n"
        "    emit \"rhs\"\n"
        "    return true\n"
        "}\n"
        "if false and rhs() { emit \"bad\" } else { emit \"ok\" }\n"
        "if false and (1 / 0) { emit \"bad\" } else { emit \"ok\" }\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['ok', 'ok']
