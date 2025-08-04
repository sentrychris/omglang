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
    """Parse a factor such as a literal, variable, or parenthesized expression."""
    tok = parser.curr_token
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

    if tok.type == 'NUMBER':
        parser.eat('NUMBER')
        return ('number', tok.value, tok.line)

    if tok.type == 'STRING':
        parser.eat('STRING')
        return ('string', tok.value, tok.line)

    if tok.type in ('TRUE', 'FALSE'):
        value = tok.type == 'TRUE'
        parser.eat(tok.type)
        return ('bool', value, tok.line)

    if tok.type == 'LBRACKET':
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
        return ('list', elements, start_tok.line)

    if tok.type == 'ID':
        id_tok = tok
        parser.eat('ID')

        if parser.curr_token.type == 'LPAREN':
            parser.eat('LPAREN')
            args = []
            if parser.curr_token.type != 'RPAREN':
                args.append(parser.expr())
                while parser.curr_token.type == 'COMMA':
                    parser.eat('COMMA')
                    args.append(parser.expr())
            parser.eat('RPAREN')
            return ('func_call', id_tok.value, args, id_tok.line)

        if parser.curr_token.type == 'LBRACKET':
            parser.eat('LBRACKET')
            start_expr = parser.expr()

            if parser.curr_token.type == 'COLON':
                parser.eat('COLON')
                end_expr = parser.expr() if parser.curr_token.type != 'RBRACKET' else None
                parser.eat('RBRACKET')
                return ('slice', id_tok.value, start_expr, end_expr, id_tok.line)

            parser.eat('RBRACKET')
            return ('index', id_tok.value, start_expr, id_tok.line)

        return ('alloc', id_tok.value, id_tok.line)

    if tok.type == 'LPAREN':
        parser.eat('LPAREN')
        node = parser.expr()
        parser.eat('RPAREN')
        return node

    raise SyntaxError(
        f"Unexpected token {tok.value} - ({tok.type}) "
        f"on line {tok.line} "
        f"in {parser.source_file}"
    )


def parse_term(parser: 'Parser') -> tuple:
    """Parse multiplication, division, and modulus expressions."""
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
    """Parse addition and subtraction expressions."""
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
    """Parse bitwise shift expressions using '<<' or '>>'."""
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
    """Parse bitwise AND expressions using '&'."""
    result = parser.shift()
    while parser.curr_token.type == 'AMP':
        tok = parser.curr_token
        parser.eat('AMP')
        result = (Op.AND_BITS, result, parser.shift(), tok.line)
    return result


def parse_bitwise_xor(parser: 'Parser') -> tuple:
    """Parse bitwise XOR expressions using '^'."""
    result = parser.bitwise_and()
    while parser.curr_token.type == 'CARET':
        tok = parser.curr_token
        parser.eat('CARET')
        result = (Op.XOR_BITS, result, parser.bitwise_and(), tok.line)
    return result


def parse_bitwise_or(parser: 'Parser') -> tuple:
    """Parse bitwise OR expressions using '|'."""
    result = parser.bitwise_xor()
    while parser.curr_token.type == 'PIPE':
        tok = parser.curr_token
        parser.eat('PIPE')
        result = (Op.OR_BITS, result, parser.bitwise_xor(), tok.line)
    return result


def parse_comparison(parser: 'Parser') -> tuple:
    """Parse comparison expressions (==, !=, <, >, <=, >=)."""
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
    """Parse logical AND expressions using the 'and' keyword."""
    result = parser.comparison()
    while parser.curr_token.type == 'AND':
        tok = parser.curr_token
        parser.eat('AND')
        result = (Op.AND, result, parser.comparison(), tok.line)
    return result


# ---- Entry point ----

def parse_expr(parser: 'Parser') -> tuple:
    """Parse an expression starting from the lowest-precedence operator."""
    return parser.logical_and()


