"""
Lexer.
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
        ('WHILE', r'\bhamsterwheel\b'),
        ('ID',       r'[A-Za-z_][A-Za-z0-9_]*'),
        ('ASSIGN',   r':='),
        ('ARROW',    r'<<'),
        ('GE',       r'>='),
        ('LE',       r'<='),
        ('EQ',       r'=='),
        ('GT',       r'>'),
        ('LT',       r'<'),
        ('LBRACE',   r'\{'),
        ('RBRACE',   r'\}'),
        ('LPAREN',   r'\('),
        ('RPAREN',   r'\)'),
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

    keywords = {
        'saywhat',
        'thingy',
        'maybe',
        'okthen'
    }

    tokens = []
    line_num = 1
    line_start = 0

    for mo in re.finditer(tok_regex, code):
        kind = mo.lastgroup
        value = mo.group()
        if kind == 'NEWLINE':
            tokens.append(Token('NEWLINE', value, line_num))
            line_num += 1
            line_start = mo.end()
            continue
        elif kind == 'SKIP':
            continue
        elif kind == 'MISMATCH':
            raise RuntimeError(f'Unexpected character {value} on line {line_num}')

        # For other tokens:
        if kind == 'NUMBER':
            tokens.append(Token('NUMBER', int(value), line_num))
        elif kind == 'STRING':
            tokens.append(Token('STRING', value[1:-1], line_num))
        elif kind == 'ID':
            if value in keywords:
                tokens.append(Token(value.upper(), value, line_num))
            else:
                tokens.append(Token('ID', value, line_num))
        else:
            tokens.append(Token(kind, value, line_num))

    tokens.append(Token('EOF', None, line_num))
    return tokens
