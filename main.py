"""
Main.
"""
import sys

from core.lexer import tokenize
from core.parser import Parser
from core.interpreter import Interpreter


if len(sys.argv) != 2:
    print("Usage: ./crsi <script.crs>")
    sys.exit(1)

with open(sys.argv[1], "r", encoding="utf-8") as f:
    code = f.read()

# Lexical Analysis: The lexer reads the raw source code and converts it into
# a stream of tokens - keywords, identifiers, operators, literals etc.
tokens = tokenize(code)

# Syntactical Analysis: The parser takes the stream of tokens and applies
# grammar rules to build an Abstract Syntax Tree (AST). The AST represents
# the hierarchical syntax of the code e.g. nested expressions, control
# structures.
parser = Parser(tokens)
ast = parser.parse()

# Evaluation: The interpreter walks the AST and executes it node by node.
# Some interpreters transform the AST into an intermediate representation
# (e.g. bytecode) and then execute that.
Interpreter().run(ast)
