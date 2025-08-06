import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(__file__)))

from core.lexer import tokenize, Token
from core.parser import Parser
from core.interpreter import Interpreter


def parse_source(source: str):
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token('EOF', None, eof_line))
    parser = Parser(tokens, token_map, '<test>')
    return parser.parse()


def test_first_class_functions_and_closures(capsys):
    source = (
        "proc inc(x) { return x + 1 }\n"
        "proc call_twice(f, x) { return f(f(x)) }\n"
        "proc make_adder(n) { proc inner(x) { return x + n } return inner }\n"
        "alloc f := inc\n"
        "emit f(2)\n"
        "emit call_twice(f, 3)\n"
        "alloc add5 := make_adder(5)\n"
        "emit add5(10)\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['3', '5', '15']
