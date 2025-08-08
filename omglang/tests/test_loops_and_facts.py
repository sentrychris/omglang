"""
Tests for loops, breaks, return statements, and facts in OMG Language.
"""
import pytest

from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_loop_and_break_runtime(capsys):
    """
    Test that loops with breaks execute correctly in OMG Language.
    """
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
    """
    Test that return statements and facts work correctly in OMG Language.
    """
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
    """
    Test that a fact that fails raises an AssertionError.
    """
    source = "facts 1 == 0\n"
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    with pytest.raises(AssertionError):
        interpreter.execute(ast)


def test_loop_allows_redeclaration(capsys):
    """Variables declared with ``alloc`` inside loops can be redeclared each iteration."""
    source = (
        "alloc i := 0\n"
        "loop i < 3 {\n"
        "    alloc x := i\n"
        "    emit x\n"
        "    i := i + 1\n"
        "}\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['0', '1', '2']
