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


def test_unary_ops_parse_and_runtime(capsys):
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
