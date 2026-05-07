"""
Tests for arithmetic, bitwise operations, and list handling in OMG Language.
"""
from omglang.operations import Op
from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_arithmetic_and_bitwise_ast_and_runtime(capsys):
    """
    Test that arithmetic and bitwise operations are parsed correctly and execute as expected.
    """
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
    """
    Test that list indexing, slicing, and built-in functions work correctly.
    """
    source = (
        "alloc nums := [1,2,3,4]\n"
        "emit nums[2]\n"
        "emit nums[1:3]\n"
        "emit length(nums)\n"
        "emit chr(65)\n"
        "emit ascii(\"B\")\n"
        "emit hex(255)\n"
        "emit binary(5)\n"
        "emit binary(-42,8)\n"
    )
    ast = parse_source(source)
    index_stmt = ast[1]
    slice_stmt = ast[2]
    assert index_stmt[1][0] == 'index'
    assert slice_stmt[1][0] == 'slice'

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['3', '[2, 3]', '4', 'A', '66', 'FF', '101', '11010110']
