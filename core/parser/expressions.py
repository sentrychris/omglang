"""
Expression parsing utilities for OMGlang.

These functions operate on a `core.parser.parser.Parser` instance and
implement the recursive descent logic for expressions, maintaining
operator precedence and associativity.
"""

from typing import TYPE_CHECKING
from core.operations import Operation

if TYPE_CHECKING:
    from core.parser import Parser


def parse_factor(parser: 'Parser') -> tuple:
    """Parse a factor (number, string, variable, or parenthesized expression).

    Returns:
        tuple: An AST node representing the factor.

    Raises:
        SyntaxError: If the syntax is invalid or unexpected.
    """
    tok = parser.curr_token
    if tok.type == 'TILDE':
        parser.eat('TILDE')
        operand = parser.factor()
        return ('unary', Operation.NOT_BITS, operand, tok.line)

    if tok.type == 'NUMBER':
        parser.eat('NUMBER')
        return ('number', tok.value, tok.line)

    elif tok.type == 'STRING':
        parser.eat('STRING')
        return ('string', tok.value, tok.line)

    elif tok.type in ('TRUE', 'FALSE'):
        value = True if tok.type == 'TRUE' else False
        parser.eat(tok.type)
        return ('bool', value, tok.line)

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
        return ('list', elements, start_tok.line)

    elif tok.type == 'ID':
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

        elif parser.curr_token.type == 'LBRACKET':
            parser.eat('LBRACKET')
            start_expr = parser.expr()

            if parser.curr_token.type == 'COLON':
                parser.eat('COLON')
                if parser.curr_token.type != 'RBRACKET':
                    end_expr = parser.expr()
                else:
                    end_expr = None
                parser.eat('RBRACKET')
                return ('slice', id_tok.value, start_expr, end_expr, id_tok.line)
            else:
                parser.eat('RBRACKET')
                return ('index', id_tok.value, start_expr, id_tok.line)

        else:
            return ('alloc', id_tok.value, id_tok.line)

    elif tok.type == 'LPAREN':
        parser.eat('LPAREN')
        node = parser.expr()
        parser.eat('RPAREN')
        return node
    else:
        raise SyntaxError(
            f"Unexpected token {tok.value} - ({tok.type}) "
            f"on line {tok.line} "
            f"in {parser.source_file}"
        )


def parse_term(parser: 'Parser') -> tuple:
    """Parse a term (factor optionally followed by an operator).

    Returns:
        tuple: An AST node representing the term.
    """
    result = parser.factor()
    while parser.curr_token.type in ('MUL', 'DIV', 'MOD'):
        op_tok = parser.curr_token
        parser.eat(op_tok.type)
        op_map = {
            'MUL': Operation.MUL,
            'DIV': Operation.DIV,
            'MOD': Operation.MOD,
        }
        result = (op_map[op_tok.type], result, parser.factor(), op_tok.line)
    return result


def parse_bitwise_or(parser: 'Parser') -> tuple:
    """Parse bitwise OR expressions using the '|' operator.

    Syntax:
        <left> | <right>

    Returns:
        tuple: ('or_bits', left_expr, right_expr, line)
    """
    result = parser.bitwise_xor()
    while parser.curr_token.type == 'PIPE':
        tok = parser.curr_token
        parser.eat('PIPE')
        result = (Operation.OR_BITS, result, parser.bitwise_xor(), tok.line)
    return result


def parse_bitwise_xor(parser: 'Parser') -> tuple:
    """Parse bitwise XOR expressions using the '^' operator.

    Syntax:
        <left> ^ <right>

    Returns:
        tuple: ('xor_bits', left_expr, right_expr, line)
    """
    result = parser.bitwise_and()
    while parser.curr_token.type == 'CARET':
        tok = parser.curr_token
        parser.eat('CARET')
        result = (Operation.XOR_BITS, result, parser.bitwise_and(), tok.line)
    return result


def parse_bitwise_and(parser: 'Parser') -> tuple:
    """Parse bitwise AND expressions using the '&' operator.

    Syntax:
        <left> & <right>

    Returns:
        tuple: ('and_bits', left_expr, right_expr, line)
    """
    result = parser.shift()
    while parser.curr_token.type == 'AMP':
        tok = parser.curr_token
        parser.eat('AMP')
        result = (Operation.AND_BITS, result, parser.shift(), tok.line)
    return result


def parse_shift(parser: 'Parser') -> tuple:
    """Parse bitwise shift expressions using '<<' or '>>'.

    Syntax:
        <left> << <right>
        <left> >> <right>

    Returns:
        tuple: ('shl' | 'shr', left_expr, right_expr, line)
    """
    result = parser.add_sub()
    while parser.curr_token.type in ('LSHIFT', 'RSHIFT'):
        tok = parser.curr_token
        if tok.type == 'LSHIFT':
            parser.eat('LSHIFT')
            result = (Operation.SHL, result, parser.add_sub(), tok.line)
        else:
            parser.eat('RSHIFT')
            result = (Operation.SHR, result, parser.add_sub(), tok.line)
    return result


def parse_add_sub(parser: 'Parser') -> tuple:
    """Parse addition and subtraction expressions.

    Syntax:
        <left> + <right>
        <left> - <right>

    Returns:
        tuple: ('add' | 'sub', left_expr, right_expr, line)
    """
    result = parser.term()
    while parser.curr_token.type in ('PLUS', 'MINUS'):
        tok = parser.curr_token
        parser.eat(tok.type)
        op_map = {
            'PLUS': Operation.ADD,
            'MINUS': Operation.SUB,
        }
        result = (op_map[tok.type], result, parser.term(), tok.line)
    return result


def parse_expr(parser: 'Parser') -> tuple:
    """Parse an expression, starting from the highest precedence (bitwise OR).

    Returns:
        tuple: The AST node representing the expression.
    """
    return parser.bitwise_or()


def parse_comparison(parser: 'Parser') -> tuple:
    """Parse a comparison expression (e.g., ==, !=, <, >, <=, >=).

    Returns:
        tuple: An AST node representing the comparison.
    """
    result = parser.expr()
    while parser.curr_token.type in ('EQ', 'NE', 'GT', 'LT', 'GE', 'LE'):
        op_tok = parser.curr_token
        parser.eat(op_tok.type)
        op_map = {
            'EQ': Operation.EQ,
            'NE': Operation.NE,
            'GT': Operation.GT,
            'LT': Operation.LT,
            'GE': Operation.GE,
            'LE': Operation.LE,
        }
        result = (op_map[op_tok.type], result, parser.expr(), op_tok.line)
    return result
