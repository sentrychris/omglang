"""
Tests for try/except error handling.
"""

import pytest

from omglang.interpreter import Interpreter
from omglang.tests.utils import parse_source


def test_try_except_handles_error():
    """
    Test error.
    """
    src = """;;;omg
alloc result := ""
try {
    length(123)
    result := "no"
} except err {
    result := err
}
"""
    ast = parse_source(src)
    interp = Interpreter("<test>")
    interp.execute(ast)
    assert "length" in interp.vars["result"]


def test_unhandled_exception_raises():
    """
    Test error
    """
    src = """;;;omg\nlength(123)\n"""
    ast = parse_source(src)
    interp = Interpreter("<test>")
    with pytest.raises(TypeError):
        interp.execute(ast)
