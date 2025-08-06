"""
Expression parsing utilities for OMGlang.

These functions operate on a `core.parser.parser.Parser` instance and
implement the recursive descent logic for expressions, maintaining
operator precedence and associativity.
"""

from typing import TYPE_CHECKING
from core.operations import Op

if TYPE_CHECKING:
    from core.parser import Parser


# ---- Highest precedence ----

def parse_factor(parser: 'Parser') -> tuple:
    """
    Parse a factor such as a literal, variable, or parenthesized expression.
    This handles unary operators, literals, identifiers, and parenthesized
    expressions, as well as postfix operations like function calls, indexing,
    slicing, and attribute access.

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed factor expression.

    Raises:
        SyntaxError: If an unexpected token is encountered or if the syntax is invalid.
    """
    tok = parser.curr_token

    # Unary operators
    if tok.type == 'TILDE':
        parser.eat('TILDE')
        operand = parser.factor()
        return ('unary', Op.NOT_BITS, operand, tok.line)

    if tok.type in ('PLUS', 'MINUS'):
        parser.eat(tok.type)
        op_map = {
            'PLUS': Op.ADD,
            'MINUS': Op.SUB,
        }
        return ('unary', op_map[tok.type], parser.factor(), tok.line)

    # Literals
    if tok.type == 'NUMBER':
        parser.eat('NUMBER')
        result = ('number', tok.value, tok.line)
    elif tok.type == 'STRING':
        parser.eat('STRING')
        result = ('string', tok.value, tok.line)
    elif tok.type in ('TRUE', 'FALSE'):
        value = tok.type == 'TRUE'
        parser.eat(tok.type)
        result = ('bool', value, tok.line)
    elif tok.type == 'LBRACKET':
        start_tok = tok
        parser.eat('LBRACKET')
        elements = []
        while parser.curr_token.type != 'RBRACKET':
            while parser.curr_token.type == 'NEWLINE':
                parser.eat('NEWLINE')
            if parser.curr_token.type == 'RBRACKET':
                break
            elements.append(parser.expr())
            while parser.curr_token.type == 'NEWLINE':
                parser.eat('NEWLINE')
            if parser.curr_token.type == 'COMMA':
                parser.eat('COMMA')
                while parser.curr_token.type == 'NEWLINE':
                    parser.eat('NEWLINE')
        parser.eat('RBRACKET')
        result = ('list', elements, start_tok.line)
    elif tok.type == 'LBRACE':
        start_tok = tok
        parser.eat('LBRACE')
        pairs = []
        while parser.curr_token.type != 'RBRACE':
            while parser.curr_token.type == 'NEWLINE':
                parser.eat('NEWLINE')
            if parser.curr_token.type == 'RBRACE':
                break
            key_tok = parser.curr_token
            if key_tok.type == 'STRING':
                parser.eat('STRING')
                key = key_tok.value
            elif key_tok.type == 'ID':
                parser.eat('ID')
                key = key_tok.value
            else:
                raise SyntaxError(
                    f"Invalid dict key {key_tok.value} on line {key_tok.line} in {parser.source_file}"
                )
            parser.eat('COLON')
            while parser.curr_token.type == 'NEWLINE':
                parser.eat('NEWLINE')
            value_expr = parser.expr()
            pairs.append((key, value_expr))
            while parser.curr_token.type == 'NEWLINE':
                parser.eat('NEWLINE')
            if parser.curr_token.type == 'COMMA':
                parser.eat('COMMA')
                while parser.curr_token.type == 'NEWLINE':
                    parser.eat('NEWLINE')
        parser.eat('RBRACE')
        result = ('dict', pairs, start_tok.line)
    elif tok.type == 'ID':
        id_tok = tok
        parser.eat('ID')
        result = ('ident', id_tok.value, id_tok.line)
    elif tok.type == 'LPAREN':
        parser.eat('LPAREN')
        result = parser.expr()
        parser.eat('RPAREN')
    else:
        raise SyntaxError(
            f"Unexpected token {tok.value} - ({tok.type}) "
            f"on line {tok.line} "
            f"in {parser.source_file}"
        )

    # Postfix operators: calls, indexing, slicing, attribute access
    while True:
        tok = parser.curr_token
        if tok.type == 'LPAREN':
            parser.eat('LPAREN')
            args = []
            if parser.curr_token.type != 'RPAREN':
                args.append(parser.expr())
                while parser.curr_token.type == 'COMMA':
                    parser.eat('COMMA')
                    args.append(parser.expr())
            parser.eat('RPAREN')
            result = ('func_call', result, args, tok.line)
        elif tok.type == 'LBRACKET':
            parser.eat('LBRACKET')
            start_expr = parser.expr()
            if parser.curr_token.type == 'COLON':
                parser.eat('COLON')
                end_expr = parser.expr() if parser.curr_token.type != 'RBRACKET' else None
                parser.eat('RBRACKET')
                result = ('slice', result, start_expr, end_expr, tok.line)
            else:
                parser.eat('RBRACKET')
                result = ('index', result, start_expr, tok.line)
        elif tok.type == 'DOT':
            parser.eat('DOT')
            attr_tok = parser.curr_token
            parser.validate_id_or_raise(attr_tok)
            parser.eat('ID')
            result = ('dot', result, attr_tok.value, attr_tok.line)
        else:
            break

    return result


def parse_term(parser: 'Parser') -> tuple:
    """
    Parse multiplication, division, and modulus expressions.

    Syntax:
        <factor> (('*' | '/' | '%') <factor>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed term expression.
    """
    result = parser.factor()
    while parser.curr_token.type in ('MUL', 'DIV', 'MOD'):
        op_tok = parser.curr_token
        parser.eat(op_tok.type)
        op_map = {
            'MUL': Op.MUL,
            'DIV': Op.DIV,
            'MOD': Op.MOD,
        }
        result = (op_map[op_tok.type], result, parser.factor(), op_tok.line)
    return result


def parse_add_sub(parser: 'Parser') -> tuple:
    """
    Parse addition and subtraction expressions.

    Syntax:
        <term> (('+' | '-') <term>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed addition or subtraction expression.
    """
    result = parser.term()
    while parser.curr_token.type in ('PLUS', 'MINUS'):
        tok = parser.curr_token
        parser.eat(tok.type)
        op_map = {
            'PLUS': Op.ADD,
            'MINUS': Op.SUB,
        }
        result = (op_map[tok.type], result, parser.term(), tok.line)
    return result


def parse_shift(parser: 'Parser') -> tuple:
    """
    Parse bitwise shift expressions using '<<' or '>>'.

    Syntax:
        <add_sub> (('<<' | '>>') <add_sub>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed shift expression.
    """
    result = parser.add_sub()
    while parser.curr_token.type in ('LSHIFT', 'RSHIFT'):
        tok = parser.curr_token
        if tok.type == 'LSHIFT':
            parser.eat('LSHIFT')
            result = (Op.SHL, result, parser.add_sub(), tok.line)
        else:
            parser.eat('RSHIFT')
            result = (Op.SHR, result, parser.add_sub(), tok.line)
    return result


def parse_bitwise_and(parser: 'Parser') -> tuple:
    """
    Parse bitwise AND expressions using '&'.

    Syntax:
        <shift> ('&' <shift>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed bitwise AND expression.
    """
    result = parser.shift()
    while parser.curr_token.type == 'AMP':
        tok = parser.curr_token
        parser.eat('AMP')
        result = (Op.AND_BITS, result, parser.shift(), tok.line)
    return result


def parse_bitwise_xor(parser: 'Parser') -> tuple:
    """
    Parse bitwise XOR expressions using '^'.

    Syntax:
        <bitwise_and> ('^' <bitwise_and>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed bitwise XOR expression.
    """
    result = parser.bitwise_and()
    while parser.curr_token.type == 'CARET':
        tok = parser.curr_token
        parser.eat('CARET')
        result = (Op.XOR_BITS, result, parser.bitwise_and(), tok.line)
    return result


def parse_bitwise_or(parser: 'Parser') -> tuple:
    """
    Parse bitwise OR expressions using '|'.

    Syntax:
        <bitwise_xor> ('|' <bitwise_xor>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed bitwise OR expression.
    """
    result = parser.bitwise_xor()
    while parser.curr_token.type == 'PIPE':
        tok = parser.curr_token
        parser.eat('PIPE')
        result = (Op.OR_BITS, result, parser.bitwise_xor(), tok.line)
    return result


def parse_comparison(parser: 'Parser') -> tuple:
    """
    Parse comparison expressions (==, !=, <, >, <=, >=).

    Syntax:
        <bitwise_or> (('==' | '!=' | '<' | '>' | '<=' | '>=') <bitwise_or>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed comparison expression.
    """
    result = parser.bitwise_or()
    while parser.curr_token.type in ('EQ', 'NE', 'GT', 'LT', 'GE', 'LE'):
        op_tok = parser.curr_token
        parser.eat(op_tok.type)
        op_map = {
            'EQ': Op.EQ,
            'NE': Op.NE,
            'GT': Op.GT,
            'LT': Op.LT,
            'GE': Op.GE,
            'LE': Op.LE,
        }
        result = (op_map[op_tok.type], result, parser.bitwise_or(), op_tok.line)
    return result


def parse_logical_and(parser: 'Parser') -> tuple:
    """
    Parse logical AND expressions using the 'and' keyword.

    Syntax:
        <comparison> ('and' <comparison>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed logical AND expression.
    """
    result = parser.comparison()
    while parser.curr_token.type == 'AND':
        tok = parser.curr_token
        parser.eat('AND')
        result = (Op.AND, result, parser.comparison(), tok.line)
    return result


def parse_logical_or(parser: 'Parser') -> tuple:
    """
    Parse logical OR expressions using the 'or' keyword.

    Syntax:
        <logical_and> ('or' <logical_and>)*

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed logical OR expression.
    """
    result = parser.logical_and()
    while parser.curr_token.type == 'OR':
        tok = parser.curr_token
        parser.eat('OR')
        result = (Op.OR, result, parser.logical_and(), tok.line)
    return result


# ---- Entry point ----

def parse_expr(parser: 'Parser') -> tuple:
    """
    Parse an expression starting from the lowest-precedence operator

    Args:
        parser: The parser instance.

    Returns:
        tuple: A tuple representing the parsed expression.
    """
    return parser.logical_or()
