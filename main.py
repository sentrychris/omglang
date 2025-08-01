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

script_name = sys.argv[1]
with open(script_name, "r", encoding="utf-8") as f:
    code = f.read()

try:
    # Create the interpreter and the check the script for the required header.
    interpreter = Interpreter(script_name)
    interpreter.check_header(code)

    # Lexical Analysis: The lexer reads the raw source code and converts it into
    # a stream of tokens - keywords, identifiers, operators, literals etc.
    tokens = tokenize(interpreter.strip_header(code))

    # Syntactical Analysis: The parser takes the stream of tokens and applies
    # grammar rules to build an Abstract Syntax Tree (AST). The AST represents
    # the hierarchical syntax of the code e.g. nested expressions, control
    # structures.
    parser = Parser(tokens, script_name)
    ast = parser.parse()

    # Evaluation: The interpreter walks the AST and executes it node by node.
    # Some interpreters transform the AST into an intermediate representation
    # (e.g. bytecode) and then execute that.
    interpreter.execute(ast)
except Exception as e:
    print(f"{type(e).__name__}: {e}")
