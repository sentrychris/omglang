"""
Tests for scoping rules in OMG Language
"""
from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_functions_have_fresh_env(capsys):
    """
    Test that functions have a fresh environment and do not leak variables.
    """
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
    """
    Test that global variables are visible inside functions.
    """
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


def test_functions_modify_globals(capsys):
    """
    Test that functions can modify global variables.
    """
    source = (
        "alloc x := 1\n"
        "proc f() {\n"
        "    x := 2\n"
        "}\n"
        "f()\n"
        "emit x\n"
    )
    ast = parse_source(source)
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['2']
