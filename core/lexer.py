"""
Lexer.

This is a regex-based lexer that performs single-pass tokenization of source code into a flat
list of tokens.

1. Token Definitions
Token types are defined via named regular expressions (token_specification), covering language 
constructs such as keywords (e.g. maybe, hamsterwheel), operators (e.g. :=, <<, +), literals 
(numbers, strings), and structural tokens (braces, parentheses, commas, etc.). The combined regex 
is constructed using named groups to enable type identification during matching.

2. Tokenization
Tokenization is performed with re.finditer over the full input string. Each match is assigned 
a type and value and wrapped in a Token object that also stores line number metadata. 
Whitespace is skipped, and newlines increment the line counter and produce a NEWLINE token.

3. Keyword Differentiation
The lexer differentiates between identifiers and language keywords (e.g., 'thingy') using a 
post-processing check after matching ID tokens. If the value matches a reserved keyword, the 
token type is replaced accordingly.

4. Comments
Single-line comments beginning with '#' are removed before tokenization. Only the portion 
before the comment delimiter is retained per line.
"""
import re

class Token:
    """
    Represents a lexical token with a type and value.
    """
    def __init__(self, type_, value, line):
        """
        Initialize a new token.

        Parameters:
            type_ (str): The token type.
            value (Any): The token value.
        """
        self.type = type_
        self.value = value
        self.line = line

    def __repr__(self):
        """
        Return a string representation of the token.
        """
        return f"Token({self.type}, {self.value}, line={self.line})"

def tokenize(code):
    """
    Convert a string of source code into a list of tokens.

    Parameters:
        code (str): The source code to tokenize.

    Returns:
        list[Token]: A list of Token instances.

    Raises:
        RuntimeError: If an unexpected character is encountered.
    """

    token_specification = [
        ('NUMBER',   r'\d+'),
        ('STRING',   r'"[^"\n]*"'),
        ('IF',       r'\bmaybe\b'),
        ('ELSE',     r'\bokthen\b'),
        ('WHILE',    r'\bhamsterwheel\b'),
        ('SAYWHAT',  r'\bsaywhat\b'),
        ('FACTS',   r"\bfacts\b"),
        ('FUNC',     r'\bbitchin\b'),
        ('COMMA',    r','),
        ('ID',       r'[A-Za-z_][A-Za-z0-9_]*'),
        ('ASSIGN',   r':='),
        ('ARROW',    r'<<'),
        ('LBRACE',   r'\{'),
        ('RBRACE',   r'\}'),
        ('LPAREN',   r'\('),
        ('RPAREN',   r'\)'),
        ('GE',       r'>='),
        ('LE',       r'<='),
        ('EQ',       r'=='),
        ('GT',       r'>'),
        ('LT',       r'<'),
        ('PLUS',     r'\+'),
        ('MINUS',    r'-'),
        ('MUL',      r'\*'),
        ('MOD',      r'%'),
        ('DIV',      r'/'),
        ('NEWLINE',  r'\n'),
        ('SKIP',     r'[ \t]+'),
        ('MISMATCH', r'.'),
    ]

    tok_regex = '|'.join(f'(?P<{name}>{pattern})' for name, pattern in token_specification)

    # Sharing ID regex
    # todo add consts, globals
    identifier_keywords = {
        'thingy',
        'facts'
    }

    tokens = []
    line_num = 1

    # Remove comments before tokenizing:
    code_no_comments = []
    for line in code.splitlines():
        stripped_line = line.split('#', 1)[0]  # Keep text before #
        code_no_comments.append(stripped_line)
    code = '\n'.join(code_no_comments)

    for mo in re.finditer(tok_regex, code):
        kind = mo.lastgroup
        value = mo.group()
        if kind == 'NEWLINE':
            tokens.append(Token('NEWLINE', value, line_num))
            line_num += 1
            continue

        if kind == 'SKIP':
            continue

        if kind == 'MISMATCH':
            raise RuntimeError(f'Unexpected character {value} on line {line_num}')

        # For other tokens:
        if kind == 'NUMBER':
            tokens.append(Token('NUMBER', int(value), line_num))
        elif kind == 'STRING':
            tokens.append(Token('STRING', value[1:-1], line_num))
        elif kind == 'ID':
            if value in identifier_keywords:
                tokens.append(Token(value.upper(), value, line_num))
            else:
                tokens.append(Token('ID', value, line_num))
        else:
            tokens.append(Token(kind, value, line_num))

    tokens.append(Token('EOF', None, line_num))
    # for t in tokens:
    #     print(t)
    return tokens
