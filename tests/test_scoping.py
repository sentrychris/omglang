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

def test_functions_have_fresh_env(capsys):
    source = (
        "proc inner() {\n"
        "    alloc x := 1\n"
        "    return x\n"
        "}\n"
        "proc outer() {\n"
        "    alloc x := 2\n"
        "    return inner()\n"
        "}\n"
        "emit outer()\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['1']

def test_globals_visible(capsys):
    source = (
        "alloc g := 5\n"
        "proc read_g() {\n"
        "    return g\n"
        "}\n"
        "emit read_g()\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['5']
