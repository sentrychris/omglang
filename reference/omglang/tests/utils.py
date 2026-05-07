"""
Utility functions shared across OMG Language tests.
"""
from pathlib import Path
import sys

from omglang.lexer import tokenize, Token
from omglang.parser import Parser
from omglang.interpreter import Interpreter

# Ensure the project root is on the Python path
PROJECT_ROOT = Path(__file__).resolve().parents[2]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.append(str(PROJECT_ROOT))


def parse_source(source: str):
    """
    Parse source code and return the AST.
    """
    tokens, token_map = tokenize(source)
    eof_line = tokens[-1].line if tokens else 1
    tokens.append(Token("EOF", None, eof_line))
    parser = Parser(tokens, token_map, "<test>")
    return parser.parse()


def run_file(path: Path) -> Interpreter:
    """
    Run a file and return the interpreter instance after execution.
    """
    code = path.read_text()
    interpreter = Interpreter(str(path))
    interpreter.check_header(code)
    tokens, token_map = tokenize(code)
    parser = Parser(tokens, token_map, str(path))
    ast = parser.parse()
    interpreter.execute(ast)
    return interpreter
