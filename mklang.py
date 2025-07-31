"""
MKLang
"""
import sys

from core.lexer import tokenize
from core.parser import Parser
from core.interpreter import Interpreter


if len(sys.argv) != 2:
    print("Usage: python mklang.py <script.mkl>")
    sys.exit(1)

with open(sys.argv[1], "r", encoding="utf-8") as f:
    code = f.read()

tokens = tokenize(code)
parser = Parser(tokenize(code))
Interpreter().run(parser.parse())
