"""
Tests for first-class functions and closures in OMG Language.
"""
from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_first_class_functions_and_closures(capsys):
    """
    Test that first-class functions and closures work correctly in OMG Language.
    """
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
