"""
OLI - OMG Language Interpreter

This is the main entry point for the OMG language interpreter.

Workflow:
1. The source script is read from the file specified on the command line.
2. The Interpreter verifies the script header to ensure validity.
3. The Lexer tokenizes the stripped source code into meaningful tokens.
4. The Parser processes tokens into an Abstract Syntax Tree (AST) following
   the language grammar.
5. The Interpreter walks the AST, evaluating expressions and executing statements.
"""
import sys

from core.lexer import tokenize
from core.parser import Parser
from core.interpreter import Interpreter


def print_usage():
    """
    Print usage.
    """
    print()
    print("OLI - OMG Language Interpreter")
    print()
    print("Usage:")
    print("    oli <script.omg>")
    print()
    print("Arguments:")
    print("    <script.omg>")
    print("        The path to an OMG language source file to execute. The file must")
    print("        include the required header ';;;omg' on the first non-empty line.")
    print()
    print("Example:")
    print("    oli hello.omg")

if len(sys.argv) != 2:
    print_usage()
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
    # (e.g. bytecode) and then execute that, but this is a simple tree-walk
    # interpreter.
    interpreter.execute(ast)
except Exception as e:
    print(f"{type(e).__name__}: {e}")
