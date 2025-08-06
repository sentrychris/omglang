"""
Tests for dictionary literals and usage in OMG Language.
"""
import os
import sys

from omglang.lexer import tokenize, Token
from omglang.parser import Parser
from omglang.interpreter import Interpreter

sys.path.append(os.path.dirname(os.path.dirname(__file__)))


def parse_source(source: str):
    """
    Parse the source code and return the AST.
    """
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token('EOF', None, eof_line))
    parser = Parser(tokens, token_map, '<test>')
    return parser.parse()


def test_dict_literal_and_usage():
    """
    Test that dictionary literals can be created and used correctly."""
    source = (
        "alloc person := {\n"
        "    name: \"Chris\",\n"
        "    age: 32\n"
        "}\n"
        "person.age := 33\n"
        "person[\"city\"] := \"Milton Keynes\"\n"
        "alloc key := \"age\"\n"
        "alloc company := { ceo: person }\n"
        "proc get_city(p) { return p.city }\n"
        "facts person.age == 33\n"
        "facts person[\"city\"] == \"Milton Keynes\"\n"
        "facts person[key] == 33\n"
        "facts company.ceo.name == \"Chris\"\n"
        "facts get_city(company.ceo) == \"Milton Keynes\"\n"
        "emit person.name\n"
        "emit person[\"city\"]\n"
    )
    ast = parse_source(source)
    decl = ast[0]
    assert decl[0] == 'decl'
    assert decl[2][0] == 'dict'
    interpreter = Interpreter('<test>')
    interpreter.execute(ast)
    person = interpreter.vars['person']
    assert person['age'] == 33
    assert person['city'] == 'Milton Keynes'
    company = interpreter.vars['company']
    assert company['ceo']['name'] == 'Chris'
