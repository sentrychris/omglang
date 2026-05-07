"""
Tests for function calls in OMG Language
"""
from omglang.interpreter import Interpreter

from omglang.tests.utils import parse_source


def test_call_ast_and_runtime(capsys):
    """
    Test that function calls are parsed correctly and execute as expected.
    """
    source = (
        "proc foo() { emit 42 }\n"
        "foo()\n"
        "emit foo()\n"
    )
    ast = parse_source(source)

    # The standalone call should be wrapped in an expr_stmt
    expr_stmt = ast[1]
    emit_stmt = ast[2]

    assert expr_stmt[0] == 'expr_stmt'
    assert emit_stmt[0] == 'emit'
    # The function call expressions should match (ignoring line numbers)
    call1 = expr_stmt[1]
    call2 = emit_stmt[1]
    assert call1[0] == 'func_call'
    assert call2[0] == 'func_call'
    assert call1[1][0] == call2[1][0] == 'ident'
    assert call1[1][1] == call2[1][1] == 'foo'
    assert call1[2] == call2[2]

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    captured = capsys.readouterr().out.strip().splitlines()
    assert captured == ['42', '42', 'None']
