"""Expression parsing utilities for OMGlang.

These functions operate on a :class:`~core.parser.parser.Parser` instance
and implement the recursive descent logic for expressions, maintaining
operator precedence and associativity.
"""

from core.operations import Operation


def parse_factor(parser) -> tuple:
    """Parse a factor (number, string, variable, or parenthesized expression).

    Returns:
        tuple: An AST node representing the factor.

    Raises:
        SyntaxError: If the syntax is invalid or unexpected.
    """
    tok = parser._current_token
    if tok.type == 'TILDE':
        parser._eat('TILDE')
        operand = parser._factor()
        return ('unary', Operation.NOT_BITS, operand, tok.line)

    if tok.type == 'NUMBER':
        parser._eat('NUMBER')
        return ('number', tok.value, tok.line)

    elif tok.type == 'STRING':
        parser._eat('STRING')
        return ('string', tok.value, tok.line)

    elif tok.type in ('TRUE', 'FALSE'):
        value = True if tok.type == 'TRUE' else False
        parser._eat(tok.type)
        return ('bool', value, tok.line)

    elif tok.type == 'LBRACKET':
        start_tok = tok
        parser._eat('LBRACKET')
        elements = []
        while parser._current_token.type != 'RBRACKET':
            while parser._current_token.type == 'NEWLINE':
                parser._eat('NEWLINE')
            if parser._current_token.type == 'RBRACKET':
                break
            elements.append(parser._expr())
            while parser._current_token.type == 'NEWLINE':
                parser._eat('NEWLINE')
            if parser._current_token.type == 'COMMA':
                parser._eat('COMMA')
                while parser._current_token.type == 'NEWLINE':
                    parser._eat('NEWLINE')
        parser._eat('RBRACKET')
        return ('list', elements, start_tok.line)

    elif tok.type == 'ID':
        id_tok = tok
        parser._eat('ID')

        if parser._current_token.type == 'LPAREN':
            parser._eat('LPAREN')
            args = []
            if parser._current_token.type != 'RPAREN':
                args.append(parser._expr())
                while parser._current_token.type == 'COMMA':
                    parser._eat('COMMA')
                    args.append(parser._expr())
            parser._eat('RPAREN')
            return ('func_call', id_tok.value, args, id_tok.line)

        elif parser._current_token.type == 'LBRACKET':
            parser._eat('LBRACKET')
            start_expr = parser._expr()

            if parser._current_token.type == 'COLON':
                parser._eat('COLON')
                if parser._current_token.type != 'RBRACKET':
                    end_expr = parser._expr()
                else:
                    end_expr = None
                parser._eat('RBRACKET')
                return ('slice', id_tok.value, start_expr, end_expr, id_tok.line)
            else:
                parser._eat('RBRACKET')
                return ('index', id_tok.value, start_expr, id_tok.line)

        else:
            return ('alloc', id_tok.value, id_tok.line)

    elif tok.type == 'LPAREN':
        parser._eat('LPAREN')
        node = parser._expr()
        parser._eat('RPAREN')
        return node
    else:
        raise SyntaxError(
            f"Unexpected token {tok.value} - ({tok.type}) "
            f"on line {tok.line} "
            f"in {parser._source_file}"
        )


def parse_term(parser) -> tuple:
    """Parse a term (factor optionally followed by an operator).

    Returns:
        tuple: An AST node representing the term.
    """
    result = parser._factor()
    while parser._current_token.type in ('MUL', 'DIV', 'MOD'):
        op_tok = parser._current_token
        parser._eat(op_tok.type)
        op_map = {
            'MUL': Operation.MUL,
            'DIV': Operation.DIV,
            'MOD': Operation.MOD,
        }
        result = (op_map[op_tok.type], result, parser._factor(), op_tok.line)
    return result


def parse_bitwise_or(parser) -> tuple:
    """Parse bitwise OR expressions using the '|' operator.

    Syntax:
        <left> | <right>

    Returns:
        tuple: ('or_bits', left_expr, right_expr, line)
    """
    result = parser._bitwise_xor()
    while parser._current_token.type == 'PIPE':
        tok = parser._current_token
        parser._eat('PIPE')
        result = (Operation.OR_BITS, result, parser._bitwise_xor(), tok.line)
    return result


def parse_bitwise_xor(parser) -> tuple:
    """Parse bitwise XOR expressions using the '^' operator.

    Syntax:
        <left> ^ <right>

    Returns:
        tuple: ('xor_bits', left_expr, right_expr, line)
    """
    result = parser._bitwise_and()
    while parser._current_token.type == 'CARET':
        tok = parser._current_token
        parser._eat('CARET')
        result = (Operation.XOR_BITS, result, parser._bitwise_and(), tok.line)
    return result


def parse_bitwise_and(parser) -> tuple:
    """Parse bitwise AND expressions using the '&' operator.

    Syntax:
        <left> & <right>

    Returns:
        tuple: ('and_bits', left_expr, right_expr, line)
    """
    result = parser._shift()
    while parser._current_token.type == 'AMP':
        tok = parser._current_token
        parser._eat('AMP')
        result = (Operation.AND_BITS, result, parser._shift(), tok.line)
    return result


def parse_shift(parser) -> tuple:
    """Parse bitwise shift expressions using '<<' or '>>'.

    Syntax:
        <left> << <right>
        <left> >> <right>

    Returns:
        tuple: ('shl' | 'shr', left_expr, right_expr, line)
    """
    result = parser._add_sub()
    while parser._current_token.type in ('LSHIFT', 'RSHIFT'):
        tok = parser._current_token
        if tok.type == 'LSHIFT':
            parser._eat('LSHIFT')
            result = (Operation.SHL, result, parser._add_sub(), tok.line)
        else:
            parser._eat('RSHIFT')
            result = (Operation.SHR, result, parser._add_sub(), tok.line)
    return result


def parse_add_sub(parser) -> tuple:
    """Parse addition and subtraction expressions.

    Syntax:
        <left> + <right>
        <left> - <right>

    Returns:
        tuple: ('add' | 'sub', left_expr, right_expr, line)
    """
    result = parser._term()
    while parser._current_token.type in ('PLUS', 'MINUS'):
        tok = parser._current_token
        parser._eat(tok.type)
        op_map = {
            'PLUS': Operation.ADD,
            'MINUS': Operation.SUB,
        }
        result = (op_map[tok.type], result, parser._term(), tok.line)
    return result


def parse_expr(parser) -> tuple:
    """Parse an expression, starting from the highest precedence (bitwise OR).

    Returns:
        tuple: The AST node representing the expression.
    """
    return parser._bitwise_or()


def parse_comparison(parser) -> tuple:
    """Parse a comparison expression (e.g., ==, !=, <, >, <=, >=).

    Returns:
        tuple: An AST node representing the comparison.
    """
    result = parser._expr()
    while parser._current_token.type in ('EQ', 'NE', 'GT', 'LT', 'GE', 'LE'):
        op_tok = parser._current_token
        parser._eat(op_tok.type)
        op_map = {
            'EQ': Operation.EQ,
            'NE': Operation.NE,
            'GT': Operation.GT,
            'LT': Operation.LT,
            'GE': Operation.GE,
            'LE': Operation.LE,
        }
        result = (op_map[op_tok.type], result, parser._expr(), op_tok.line)
    return result
