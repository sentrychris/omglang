"""Statement parsing utilities for OMGlang.

These functions operate on a :class:`~core.parser.parser.Parser` instance
and handle the various statement forms in the language such as blocks,
conditionals, loops, and function definitions.
"""


def parse_block(parser) -> tuple:
    """Parse a block of statements enclosed in braces.

    Returns:
        tuple: A ('block', list_of_statements, line_number) AST node.
    """
    tok = parser._current_token
    parser._eat('LBRACE')
    statements = []
    while parser._current_token.type != 'RBRACE':
        while parser._current_token.type == 'NEWLINE':
            parser._eat('NEWLINE')
        statements.append(parser._statement())
        while parser._current_token.type == 'NEWLINE':
            parser._eat('NEWLINE')
    parser._eat('RBRACE')
    return ('block', statements, tok.line)


def parse_statement(parser) -> tuple:
    """Parse a single statement.

    Returns:
        tuple: A tuple representing the statement AST node.

    Raises:
        SyntaxError: If the syntax is invalid or unexpected.
    """
    tok = parser._current_token
    if tok.type == "FACTS":
        return parser._parse_facts()
    elif tok.type == 'ECHO':
        return parser._parse_echo()
    elif tok.type == 'IF':
        return parser._parse_if()
    elif tok.type == 'WHILE':
        return parser._parse_while()
    elif tok.type == 'BREAK':
        return parser._parse_break()
    elif tok.type == 'FUNC':
        return parser._parse_func_def()
    elif tok.type == 'ALLOC':
        return parser._parse_assignment()
    elif tok.type == 'ID':
        if (
            parser._position + 1 < len(parser._tokens)
            and parser._tokens[parser._position + 1].type == 'ASSIGN'
        ):
            return parser._parse_reassignment()
        expr_node = parser._factor()
        return ('expr_stmt', expr_node, expr_node[-1])
    elif tok.type == 'RETURN':
        return parser._parse_return()
    else:
        raise SyntaxError(
            f"Unexpected token {tok.type} "
            f"on line {tok.line} "
            f"in {parser._source_file}"
        )


def parse_facts(parser) -> tuple:
    """Parse a 'facts' statement.

    Syntax:
        facts <expression>

    Returns:
        tuple: ('facts', expression_node, line_number)

    Raises:
        SyntaxError: If the expression is malformed.
    """
    tok = parser._current_token
    parser._eat("FACTS")
    expr_node = parser._comparison()
    return ("facts", expr_node, tok.line)


def parse_echo(parser) -> tuple:
    """Parse a 'emit' (echo) statement.

    Syntax:
        emit <expression>

    Returns:
        tuple: ('emit', expression_node, line_number)

    Raises:
        SyntaxError: If the expression is malformed.
    """
    tok = parser._current_token
    parser._eat("ECHO")
    expr_node = parser._expr()
    return ("emit", expr_node, tok.line)


def parse_if(parser) -> tuple:
    """Parse a conditional 'if' statement with optional elif and else blocks."""
    tok = parser._current_token
    parser._eat("IF")
    condition = parser._comparison()
    then_block = parser._block()

    elif_cases = []
    while parser._current_token.type == 'ELIF':
        parser._eat('ELIF')
        cond = parser._comparison()
        block = parser._block()
        elif_cases.append((cond, block))

    else_block = None
    if parser._current_token.type == 'ELSE':
        parser._eat('ELSE')
        else_block = parser._block()

    tail = else_block
    for cond_node, block_node in reversed(elif_cases):
        cond_line = cond_node[-1] if isinstance(cond_node, tuple) else tok.line
        tail = ('if', cond_node, block_node, tail, cond_line)

    return ('if', condition, then_block, tail, tok.line)


def parse_while(parser) -> tuple:
    """Parse a 'loop' (while) loop."""
    tok = parser._current_token
    parser._eat('WHILE')
    condition = parser._comparison()
    body = parser._block()
    return ('loop', condition, body, tok.line)


def parse_break(parser) -> tuple:
    """Parse a 'break' control statement."""
    tok = parser._current_token
    parser._eat("BREAK")
    return ("break", tok.line)


def parse_func_def(parser) -> tuple:
    """Parse a function definition."""
    start_tok = parser._current_token
    parser._eat('FUNC')
    func_name = parser._current_token.value
    parser._eat('ID')
    parser._eat('LPAREN')
    params = []
    if parser._current_token.type != 'RPAREN':
        params.append(parser._current_token.value)
        parser._eat('ID')
        while parser._current_token.type == 'COMMA':
            parser._eat('COMMA')
            params.append(parser._current_token.value)
            parser._eat('ID')
    parser._eat('RPAREN')
    body = parser._block()
    return ('func_def', func_name, params, body, start_tok.line)


def parse_return(parser) -> tuple:
    """Parse a 'return' statement."""
    tok = parser._current_token
    parser._eat("RETURN")
    expr_node = parser._expr()
    return ("return", expr_node, tok.line)


def parse_reassignment(parser) -> tuple:
    """Parse reassignment of an existing variable."""
    id_tok = parser._current_token
    parser._eat('ID')
    parser._eat('ASSIGN')
    expr_node = parser._expr()
    return ('assign', id_tok.value, expr_node, id_tok.line)


def parse_assignment(parser) -> tuple:
    """Parse a 'alloc' variable assignment."""
    parser._eat('ALLOC')
    id_tok = parser._current_token
    if id_tok.type != 'ID':
        raise SyntaxError(
            f"Expected identifier after 'alloc' "
            f"on line {id_tok.line} "
            f"in {parser._source_file}"
        )
    var_name = id_tok.value
    parser._eat('ID')
    if parser._current_token.type != 'ASSIGN':
        raise SyntaxError(
            f"Expected ':=' after variable name "
            f"on line {parser._current_token.line} in {parser._source_file}"
        )
    parser._eat('ASSIGN')
    expr_node = parser._expr()
    return ('assign', var_name, expr_node, id_tok.line)
