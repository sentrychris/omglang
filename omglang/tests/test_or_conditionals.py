import os
import sys

from omglang.core.lexer import tokenize, Token
from omglang.core.parser import Parser
from omglang.core.operations import Op
from omglang.core.interpreter import Interpreter

sys.path.append(os.path.dirname(os.path.dirname(__file__)))

def parse_source(source: str):
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token('EOF', None, eof_line))
    parser = Parser(tokens, token_map, '<test>')
    return parser.parse()


def test_if_or_ast_and_runtime(capsys):
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
