"""
Lexer.

This is a regex-based lexer that performs single-pass tokenization of source code into a flat
list of tokens.

1. Token Definitions
Token types are defined via named regular expressions (token_specification), covering language 
constructs such as keywords (e.g. maybe, roundabout), operators (e.g. :=, <<, +), literals 
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

    token_specification: list[Token] = [
        # Literals
        ('BINARY',    r'0b[01]+'),
        ('NUMBER',    r'\d+'),
        ('STRING',    r'"([^"\\]|\\.)*"'),
        ('TRUE',      r'\btrue\b'),
        ('FALSE',     r'\bfalse\b'),

        # Keywords
        ('IF',        r'\bmaybe\b'),
        ('ELIF',      r'\boractually'),
        ('ELSE',      r'\bokthen\b'),
        ('WHILE',     r'\broundabout\b'),
        ('ECHO',      r'\bwoah\b'),
        ('FACTS',     r'\bfacts\b'),
        ('FUNC',      r'\bbitchin\b'),
        ('RETURN',    r'\bgimme\b'),

        # Chain
        ('CHAIN',     r'<-'),

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
        ('COLON',     r':'),

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

    identifier_keywords = {
        'thingy',
    }

    tokens = []
    line_num = 1

    # Remove comments before tokenizing:
    code_no_comments = []
    for line in code.splitlines():
        stripped_line = line.split('#', 1)[0]  # Keep text before #
        code_no_comments.append(stripped_line)
    code = '\n'.join(code_no_comments)

    for match_obj in re.finditer(tok_regex, code):
        kind = match_obj.lastgroup
        value = match_obj.group()

        if kind == 'NEWLINE':
            tokens.append(Token('NEWLINE', value, line_num))
            line_num += 1
            continue
        if kind == 'SKIP':
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
            if value in identifier_keywords:
                tokens.append(Token(value.upper(), value, line_num))
            else:
                tokens.append(Token('ID', value, line_num))
        else:
            tokens.append(Token(kind, value, line_num))

    tokens.append(Token('EOF', None, line_num))
    # for t in tokens:
    #     print(t)
    return tokens, token_map_literals
