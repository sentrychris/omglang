"""
Tests for declaration and assignment in OMG Language.
"""
from omglang.interpreter import Interpreter
from omglang.exceptions import UndefinedVariableException

from omglang.tests.utils import parse_source


def test_decl_and_assign_ast_and_runtime():
    """
    Test that declaration and assignment are parsed correctly and execute as expected.
    """
    source = (
        "alloc x := 5\n"
        "x := x + 1\n"
    )
    ast = parse_source(source)
    decl, assign = ast
    assert decl[0] == 'decl'
    assert assign[0] == 'assign'

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    assert interpreter.vars['x'] == 6


def test_assign_without_decl_raises():
    """
    Test that assigning to an undeclared variable raises an exception.
    """
    source = "x := 5\n"
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    try:
        interpreter.execute(ast)
    except UndefinedVariableException:
        pass
    else:
        raise AssertionError('expected UndefinedVariableException')
