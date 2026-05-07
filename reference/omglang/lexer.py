"""Lexer for OMGlang.

This lexer performs a single pass over the source code using a combined
regular expression of named groups. Each match yields a :class:`Token`
containing its type, value and source line number.

Tokens cover literals (numbers, strings, booleans), keywords (``if``, ``loop``,
``emit`` …), operators and delimiters. Comment text beginning with ``#`` or
enclosed within ``/** … */`` docblocks is skipped during tokenization so line
numbers remain accurate. When present, the required ``;;;omg`` header is also
stripped before lexing.


File: lexer.py
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © 2025 Chris Rowles. All rights reserved.
Version: 0.1.1
License: MIT
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

    def __repr__(self) -> str:
        """
        Return a string representation of the token.
        """
        return f"Token({self.type}, {self.value}, line={self.line})"


def tokenize(code) -> tuple[list[Token], dict[str, str]]:
    """
    Convert a string of source code into a list of tokens.

    Parameters:
        code (str): The source code to tokenize.

    Returns:
        list[Token]: A list of Token instances.
        dict[str, str]: A dict containing mapped token-values.

    Raises:
        RuntimeError: If an unexpected character is encountered.
    """

    # Strip header before tokenizing
    lines = code.splitlines()
    for i, line in enumerate(lines):
        if line.strip() == "":
            continue
        if line.strip() == ";;;omg":
            code = "\n".join(lines[i + 1:])
        break

    token_specification: list[Token] = [
        # Literals
        ('BINARY',    r'0b[01]+'),
        ('NUMBER',    r'\d+'),
        ('STRING',    r'"([^"\\]|\\.)*"'),
        ('TRUE',      r'\btrue\b'),
        ('FALSE',     r'\bfalse\b'),

        # Keywords
        ('IF',        r'\bif\b'),
        ('ELIF',      r'\belif\b'),
        ('ELSE',      r'\belse\b'),
        ('LOOP',      r'\bloop\b'),
        ('BREAK',     r'\bbreak\b'),
        ('EMIT',      r'\bemit\b'),
        ('IMPORT',    r'\bimport\b'),
        ('AS',        r'\bas\b'),
        ('FACTS',     r'\bfacts\b'),
        ('FUNC',      r'\bproc\b'),
        ('RETURN',    r'\breturn\b'),
        ('AND',       r'\band\b'),
        ('OR',        r'\bor\b'),
        ('ALLOC',     r'\balloc\b'),
        ('TRY',       r'\btry\b'),
        ('EXCEPT',    r'\bexcept\b'),

        # Chain
        # ('CHAIN',     r''),

        # Identifiers
        ('ID',        r'[A-Za-z_][A-Za-z0-9_]*'),

        # Assignment
        ('ASSIGN',    r':='),

        # Delimiters
        ('LBRACE',    r'\{'),
        ('RBRACE',    r'\}'),
        ('LPAREN',    r'\('),
        ('RPAREN',    r'\)'),
        ('LBRACKET',  r'\['),
        ('RBRACKET',  r'\]'),
        ('COMMA',     r','),
        ('DOT',       r'\.'),
        ('COLON',     r':'),

        # Comments
        ('DOCBLOCK',  r'/\*\*(?:.|\n)*?\*/'),

        # Arithmetic operators
        ('PLUS',      r'\+'),
        ('MINUS',     r'-'),
        ('MUL',       r'\*'),
        ('MOD',       r'%'),
        ('DIV',       r'/'),

        # Bitwise operators
        ('LSHIFT',    r'<<'),
        ('RSHIFT',    r'>>'),
        ('AMP',       r'&'),
        ('PIPE',      r'\|'),
        ('CARET',     r'\^'),
        ('TILDE',     r'~'),

        # Comparison operators
        ('GE',        r'>='),
        ('LE',        r'<='),
        ('EQ',        r'=='),
        ('NE',        r'!='),
        ('GT',        r'>'),
        ('LT',        r'<'),

        # Miscellaneous
        ('COMMENT',   r'\#[^\n]*'),
        ('NEWLINE',   r'\n'),
        ('SKIP',      r'[ \t]+'),
        ('MISMATCH',  r'.'),
    ]

    token_map_literals = {}
    for name, pattern in token_specification:
        try:
            literal = re.compile(pattern).pattern
            literal = literal.replace(r'\b', '')
            if re.match(r'^[\\\w{}()<>=:%+\-*/]+$', literal):
                unescaped = re.sub(r'\\', '', literal)
                token_map_literals[unescaped] = name
        except re.error:
            pass

    tok_regex = '|'.join(f'(?P<{name}>{pattern})' for name, pattern in token_specification)

    tokens = []
    line_num = 2  # 1 contains the stripped header

    for match_obj in re.finditer(tok_regex, code):
        kind = match_obj.lastgroup
        value = match_obj.group()

        if kind == 'NEWLINE':
            tokens.append(Token('NEWLINE', value, line_num))
            line_num += 1
            continue
        if kind == 'SKIP':
            continue
        if kind == 'COMMENT':
            continue
        if kind == 'DOCBLOCK':
            line_num += value.count('\n')
            continue
        if kind == 'MISMATCH':
            raise RuntimeError(f'Unexpected character {value} on line {line_num}')

        elif kind == 'BINARY':
            value = int(value, 2)
            tokens.append(Token('NUMBER', value, line_num))
        elif kind == 'NUMBER':
            tokens.append(Token('NUMBER', int(value), line_num))
        elif kind == 'STRING':
            value = value[1:-1]
            value = bytes(value, 'utf-8').decode('unicode_escape')
            tokens.append(Token('STRING', value, line_num))
        elif kind == 'ID':
            tokens.append(Token('ID', value, line_num))
        else:
            tokens.append(Token(kind, value, line_num))

    tokens.append(Token('EOF', None, line_num))
    # for t in tokens:
    #     print(t)
    return tokens, token_map_literals
