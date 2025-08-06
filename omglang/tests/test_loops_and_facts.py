import os
import sys
import pytest

from omglang.lexer import tokenize, Token
from omglang.parser import Parser
from omglang.interpreter import Interpreter

sys.path.append(os.path.dirname(os.path.dirname(__file__)))

def parse_source(source: str):
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token('EOF', None, eof_line))
    parser = Parser(tokens, token_map, '<test>')
    return parser.parse()


def test_loop_and_break_runtime(capsys):
    source = (
        "alloc i := 0\n"
        "loop i < 5 {\n"
        "    emit i\n"
        "    i := i + 1\n"
        "    if i == 3 { break }\n"
        "}\n"
    )
    ast = parse_source(source)
    assert ast[1][0] == 'loop'

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['0', '1', '2']


def test_return_and_facts(capsys):
    source = (
        "proc inc(x) { return x + 1 }\n"
        "alloc v := inc(5)\n"
        "emit v\n"
        "facts v == 6\n"
    )
    ast = parse_source(source)
    func_def = ast[0]
    assert func_def[0] == 'func_def'
    block = func_def[3]
    assert block[0] == 'block'
    assert block[1][0][0] == 'return'

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['6']


def test_facts_failure():
    source = "facts 1 == 0\n"
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    with pytest.raises(AssertionError):
        interpreter.execute(ast)

