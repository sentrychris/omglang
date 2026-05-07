"""Tests for docblock comments in OMG Language."""
from omglang.interpreter import Interpreter
from omglang.tests.utils import parse_source


def test_docblock_comment_ignored(capsys):
    """Ensure multiline docblock comments are skipped by the lexer."""
    source = (
        "/**\n"
        " * Docblock comment\n"
        " * spanning multiple lines\n"
        " */\n"
        "emit \"hi\"\n"
    )
    ast = parse_source(source)
    assert len(ast) == 1
    assert ast[0][0] == 'emit'

    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    assert capsys.readouterr().out == "hi\n"
