import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(__file__)))

from core.lexer import tokenize, Token
from core.parser import Parser
from core.operations import Op
from core.interpreter import Interpreter


def parse_source(source: str):
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token('EOF', None, eof_line))
    parser = Parser(tokens, token_map, '<test>')
    return parser.parse()


def test_if_and_ast_and_runtime(capsys):
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
