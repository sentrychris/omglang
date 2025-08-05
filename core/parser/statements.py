"""
Statement parsing utilities for OMGlang.

These functions operate on a `core.parser.parser.Parser` instance and
handle the various statement forms in the language such as blocks,
conditionals, loops, and function definitions.
"""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from core.parser import Parser


def parse_block(parser: 'Parser') -> tuple:
    """
    Parse a block of statements enclosed in braces.

    Args:
        parser: The parser instance.

    Returns:
        tuple: ('block', list_of_statements, line_number)
    """
    tok = parser.curr_token
    parser.eat('LBRACE')
    statements = []
    while parser.curr_token.type != 'RBRACE':
        while parser.curr_token.type == 'NEWLINE':
            parser.eat('NEWLINE')
        statements.append(parser.statement())
        while parser.curr_token.type == 'NEWLINE':
            parser.eat('NEWLINE')
    parser.eat('RBRACE')
    return ('block', statements, tok.line)


def parse_statement(parser: 'Parser') -> tuple:
    """
    Parse a single statement.

    Args:
        parser: The parser instance.

    Returns:
        tuple: tuple: representing the AST node.
    """
    tok = parser.curr_token
    if tok.type == "FACTS":
        return parser.parse_facts()
    elif tok.type == 'EMIT':
        return parser.parse_emit()
    elif tok.type == 'IF':
        return parser.parse_if()
    elif tok.type == 'LOOP':
        return parser.parse_loop()
    elif tok.type == 'BREAK':
        return parser.parse_break()
    elif tok.type == 'FUNC':
        return parser.parse_func_def()
    elif tok.type == 'ALLOC':
        return parser.parse_declaration()
    elif tok.type == 'ID':
        if (
            parser.position + 1 < len(parser.tokens)
            and parser.tokens[parser.position + 1].type == 'ASSIGN'
        ):
            return parser.parse_reassignment()
        expr_node = parser.factor()
        return ('expr_stmt', expr_node, expr_node[-1])
    elif tok.type == 'RETURN':
        return parser.parse_return()
    else:
        raise SyntaxError(
            f"Unexpected token {tok.type} "
            f"on line {tok.line} "
            f"in {parser.source_file}"
        )


def parse_facts(parser: 'Parser') -> tuple:
    """
    Parse a 'facts' statement.

    Syntax:
        facts <expression>

    Args:
        parser: The parser instance.

    Returns:
        tuple: tuple: representing the AST node.
    """
    tok = parser.curr_token
    parser.eat("FACTS")
    expr_node = parser.expr()
    return ("facts", expr_node, tok.line)


def parse_emit(parser: 'Parser') -> tuple:
    """
    Parse an 'emit' statement.

    Syntax:
        emit <expression>

    Args:
        parser: The parser instance.

    Returns:
        tuple: ('emit', expression_node, line_number)
    """
    tok = parser.curr_token
    parser.eat("EMIT")
    expr_node = parser.expr()
    return ("emit", expr_node, tok.line)


def parse_if(parser: 'Parser') -> tuple:
    """
    Parse a conditional 'if' statement with optional elif and else blocks.

    Args:
        parser: The parser instance.

    Returns:
        tuple: tuple: representing the AST node.
    """
    tok = parser.curr_token
    parser.eat("IF")
    condition = parser.expr()
    then_block = parser.block()

    elif_cases = []
    while parser.curr_token.type == 'ELIF':
        parser.eat('ELIF')
        cond = parser.expr()
        block = parser.block()
        elif_cases.append((cond, block))

    else_block = None
    if parser.curr_token.type == 'ELSE':
        parser.eat('ELSE')
        else_block = parser.block()

    tail = else_block
    for cond_node, block_node in reversed(elif_cases):
        cond_line = cond_node[-1] if isinstance(cond_node, tuple) else tok.line
        tail = ('if', cond_node, block_node, tail, cond_line)

    return ('if', condition, then_block, tail, tok.line)


def parse_loop(parser: 'Parser') -> tuple:
    """
    Parse a 'loop' statement.

    Returns:
        tuple: representing the AST node.
    """
    tok = parser.curr_token
    parser.eat('LOOP')
    condition = parser.expr()
    body = parser.block()
    return ('loop', condition, body, tok.line)


def parse_break(parser: 'Parser') -> tuple:
    """
    Parse a 'break' control statement.

    Args:
        parser: The parser instance.

    Returns:
        tuple: representing the AST node.
    """
    tok = parser.curr_token
    parser.eat("BREAK")
    return ("break", tok.line)


def parse_func_def(parser: 'Parser') -> tuple:
    """
    Parse a function definition.

    Args:
        parser: The parser instance.

    Returns:
        tuple: representing the AST node.
    """
    start_tok = parser.curr_token
    parser.eat('FUNC')
    func_name = parser.curr_token.value
    parser.eat('ID')
    parser.eat('LPAREN')
    params = []
    if parser.curr_token.type != 'RPAREN':
        params.append(parser.curr_token.value)
        parser.eat('ID')
        while parser.curr_token.type == 'COMMA':
            parser.eat('COMMA')
            params.append(parser.curr_token.value)
            parser.eat('ID')
    parser.eat('RPAREN')
    body = parser.block()
    return ('func_def', func_name, params, body, start_tok.line)


def parse_return(parser: 'Parser') -> tuple:
    """
    Parse a 'return' statement.

    Args:
        parser: The parser instance.

    Returns:
        tuple: representing the AST node.
    """
    tok = parser.curr_token
    parser.eat("RETURN")
    expr_node = parser.expr()
    return ("return", expr_node, tok.line)


def parse_reassignment(parser: 'Parser') -> tuple:
    """
    Parse reassignment of an existing variable.

    Args:
        parser: The parser instance.

    Returns:
        tuple: representing the AST node.
    """
    id_tok = parser.curr_token
    parser.eat('ID')
    parser.eat('ASSIGN')
    expr_node = parser.expr()
    return ('assign', id_tok.value, expr_node, id_tok.line)


def parse_declaration(parser: 'Parser') -> tuple:
    """
    Parse an ``alloc`` variable declaration.

    Args:
        parser: The parser instance.

    Returns:
        tuple: ('decl', name, expr, line)
    """
    parser.eat('ALLOC')
    id_tok = parser.curr_token
    if id_tok.type != 'ID':
        raise SyntaxError(
            f"Expected identifier after 'alloc' "
            f"on line {id_tok.line} "
            f"in {parser.source_file}"
        )
    var_name = id_tok.value
    parser.eat('ID')
    if parser.curr_token.type != 'ASSIGN':
        raise SyntaxError(
            f"Expected ':=' after variable name "
            f"on line {parser.curr_token.line} in {parser.source_file}"
        )
    parser.eat('ASSIGN')
    expr_node = parser.expr()
    return ('decl', var_name, expr_node, id_tok.line)
