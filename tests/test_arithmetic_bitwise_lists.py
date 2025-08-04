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


def test_arithmetic_and_bitwise_ast_and_runtime(capsys):
    source = (
        "emit 1 + 2 * 3\n"
        "emit 10 / 2 - 3\n"
        "emit 5 % 2\n"
        "emit 1 << 2\n"
        "emit 8 >> 1\n"
        "emit 6 & 3\n"
        "emit 6 | 3\n"
        "emit 6 ^ 3\n"
        "emit ~1\n"
    )
    ast = parse_source(source)
    first_expr = ast[0][1]
    assert first_expr[0] == Op.ADD
    assert first_expr[2][0] == Op.MUL

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['7', '2', '1', '4', '4', '2', '7', '5', '-2']


def test_list_index_slice_and_builtins(capsys):
    source = (
        "alloc nums := [1,2,3,4]\n"
        "emit nums[2]\n"
        "emit nums[1:3]\n"
        "emit length(nums)\n"
        "emit chr(65)\n"
        "emit ascii(\"B\")\n"
        "emit hex(255)\n"
        "emit binary(5)\n"
    )
    ast = parse_source(source)
    index_stmt = ast[1]
    slice_stmt = ast[2]
    assert index_stmt[1][0] == 'index'
    assert slice_stmt[1][0] == 'slice'

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['3', '[2, 3]', '4', 'A', '66', 'FF', '101']

