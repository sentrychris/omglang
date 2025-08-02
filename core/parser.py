"""
Parser.

This is a recursive descent parser that operates in a mostly LL(1) fashion.

1. Parsing
The parser implements a set of mutually recursive functions (_factor(), _term(), _expr(), 
_comparison(), _statement(), _block()), each corresponding to a nonterminal in the grammar. 
These functions parse specific syntactic constructs and call one another as needed, reflecting 
the structure of the grammar.

2. Token Consumption
The parser consumes tokens manually using an eat() method, which checks the current token 
against the expected type and advances the token stream if they match.

3. LL(1) & LL(2)
Parsing decisions are typically made using a single token of lookahead (LL(1)). In limited cases,
the parser peeks ahead one additional token (e.g., self._tokens[self._position + 1] in _factor()) 
to resolve ambiguities such as distinguishing variable references from function calls. This 
occasional lookahead makes the parser technically LL(2) in those specific situations, but its 
overall behavior and structure align with LL(1) principles.
"""

class Parser:
    """
    OMGlang parser.
    """
    def __init__(self, tokens: list, token_map_literals: dict[str, str], file: str):
        """
        Initialize the parser with a list of tokens.

        Parameters:
            tokens (list): A list of Token instances.
            token_map_literals (dict): A dict of tokens mapped to their literals.
            file (str): The name of the script.
        """
        self._tokens = tokens
        self._token_map = token_map_literals
        self._reverse_token_map = {v: k for k, v in self._token_map.items()}
        self._position = 0
        self._current_token = self._tokens[self._position]
        self._source_file = file


    def _eat(self, token_type: str) -> None:
        """
        Consume the current token if it matches the expected type.

        Parameters:
            token_type (str): The expected token type.

        Raises:
            SyntaxError: If the token does not match the expected type.
        """
        if self._current_token.type == token_type:
            self._position += 1
            self._current_token = self._tokens[self._position]
        else:
            expd_value = self._reverse_token_map.get(token_type, token_type)
            act_value = self._current_token.value
            act_type = self._current_token.type
            mapped_type = self._token_map.get(act_value, None)
            mapped_hint = f" (which maps to {mapped_type})" if mapped_type else ""

            raise SyntaxError(
                f"Expected token '{expd_value}' of type {token_type}, "
                f"but got value '{act_value}' of type {act_type}{mapped_hint} "
                f"on line {self._current_token.line} in {self._source_file}"
            )


    def _factor(self) -> tuple:
        """
        Parse a factor (number, string, variable, or parenthesized expression).

        Returns:
            An AST node representing the factor.

        Raises:
            SyntaxError: If the syntax is invalid or unexpected.
        """
        tok = self._current_token
        if tok.type == 'TILDE':
            self._eat('TILDE')
            operand = self._factor()
            return ('unary', 'not_bits', operand, tok.line)

        if tok.type == 'NUMBER':
            self._eat('NUMBER')
            return ('number', tok.value, tok.line)

        elif tok.type == 'STRING':
            self._eat('STRING')
            return ('string', tok.value, tok.line)

        elif tok.type in ('TRUE', 'FALSE'):
            value = True if tok.type == 'TRUE' else False
            self._eat(tok.type)
            return ('bool', value, tok.line)

        elif tok.type == 'LBRACKET':
            start_tok = tok
            self._eat('LBRACKET')
            elements = []
            while self._current_token.type != 'RBRACKET':
                while self._current_token.type == 'NEWLINE':
                    self._eat('NEWLINE')
                if self._current_token.type == 'RBRACKET':
                    break
                elements.append(self._expr())
                while self._current_token.type == 'NEWLINE':
                    self._eat('NEWLINE')
                if self._current_token.type == 'COMMA':
                    self._eat('COMMA')
                    while self._current_token.type == 'NEWLINE':
                        self._eat('NEWLINE')
            self._eat('RBRACKET')
            return ('list', elements, start_tok.line)

        elif tok.type == 'ID':
            id_tok = tok
            self._eat('ID')

            if self._current_token.type == 'LPAREN':
                # function call
                self._eat('LPAREN')
                args = []
                if self._current_token.type != 'RPAREN':
                    args.append(self._expr())
                    while self._current_token.type == 'COMMA':
                        self._eat('COMMA')
                        args.append(self._expr())
                self._eat('RPAREN')
                return ('func_call', id_tok.value, args, id_tok.line)

            elif self._current_token.type == 'LBRACKET':
                # List indexing
                self._eat('LBRACKET')
                start_expr = self._expr()

                if self._current_token.type == 'COLON':
                    # List slicing
                    self._eat('COLON')
                    if self._current_token.type != 'RBRACKET':
                        end_expr = self._expr()
                    else:
                        end_expr = None
                    self._eat('RBRACKET')
                    return ('slice', id_tok.value, start_expr, end_expr, id_tok.line)
                else:
                    self._eat('RBRACKET')
                    return ('index', id_tok.value, start_expr, id_tok.line)

            else:
                return ('thingy', id_tok.value, id_tok.line)

        elif tok.type == 'LPAREN':
            self._eat('LPAREN')
            node = self._expr()
            self._eat('RPAREN')
            return node
        else:
            raise SyntaxError(
                f"Unexpected token {tok.value} - ({tok.type}) "
                f"on line {tok.line} "
                f"in {self._source_file}"
            )


    def _term(self) -> tuple:
        """
        Parse a term (factor optionally followed by an operator).

        Returns:
            An AST node representing the term.
        """
        result = self._factor()
        while self._current_token.type in ('MUL', 'DIV', 'MOD'):
            op_tok = self._current_token
            self._eat(op_tok.type)
            op_map = {'MUL': 'mul', 'DIV': 'div', 'MOD': 'mod'}
            result = (op_map[op_tok.type], result, self._factor(), op_tok.line)
        return result


    def _expr(self) -> tuple:
        """
        Parse an expression (term optionally followed by an operator).

        Returns:
            An AST node representing the expression.
        """
        return self._bitwise_or()


    def _bitwise_or(self) -> tuple:
        result = self._bitwise_xor()
        while self._current_token.type == 'PIPE':
            tok = self._current_token
            self._eat('PIPE')
            result = ('or_bits', result, self._bitwise_xor(), tok.line)
        return result


    def _bitwise_xor(self) -> tuple:
        result = self._bitwise_and()
        while self._current_token.type == 'CARET':
            tok = self._current_token
            self._eat('CARET')
            result = ('xor_bits', result, self._bitwise_and(), tok.line)
        return result


    def _bitwise_and(self) -> tuple:
        result = self._shift()
        while self._current_token.type == 'AMP':
            tok = self._current_token
            self._eat('AMP')
            result = ('and_bits', result, self._shift(), tok.line)
        return result


    def _shift(self) -> tuple:
        result = self._add_sub()
        while self._current_token.type in ('LSHIFT', 'RSHIFT'):
            tok = self._current_token
            if tok.type == 'LSHIFT':
                self._eat('LSHIFT')
                result = ('shl', result, self._add_sub(), tok.line)
            else:
                self._eat('RSHIFT')
                result = ('shr', result, self._add_sub(), tok.line)
        return result


    def _add_sub(self) -> tuple:
        result = self._term()
        while self._current_token.type in ('PLUS', 'MINUS'):
            tok = self._current_token
            self._eat(tok.type)
            op_map = {'PLUS': 'add', 'MINUS': 'sub'}
            result = (op_map[tok.type], result, self._term(), tok.line)
        return result


    def _comparison(self) -> tuple:
        """
        Parse a comparison expression (e.g., ==, <, >, <=, >=).

        Returns:
            An AST node representing the comparison.
        """
        result = self._expr()
        while self._current_token.type in ('EQ', 'GT', 'LT', 'GE', 'LE'):
            op_tok = self._current_token
            self._eat(op_tok.type)
            op_map = {'EQ': 'eq', 'GT': 'gt', 'LT': 'lt', 'GE': 'ge', 'LE': 'le'}
            result = (op_map[op_tok.type], result, self._expr(), op_tok.line)
        return result


    def _block(self) -> tuple:
        """
        Parse a block of statements enclosed in braces.

        Returns:
            A ('block', list_of_statements, line_number) AST node.
        """
        tok = self._current_token
        self._eat('LBRACE')
        statements = []
        while self._current_token.type != 'RBRACE':
            while self._current_token.type == 'NEWLINE':
                self._eat('NEWLINE')
            statements.append(self._statement())
            while self._current_token.type == 'NEWLINE':
                self._eat('NEWLINE')
        self._eat('RBRACE')
        return ('block', statements, tok.line)


    def _statement(self) -> tuple:
        """
        Parse a single statement (woah, thingy assignment, maybe, or while).

        Returns:
            A tuple representing the statement AST node.

        Raises:
            SyntaxError: If the syntax is invalid or unexpected.
        """
        tok = self._current_token
        if tok.type == "FACTS":
            return self._parse_facts()
        elif tok.type == 'ECHO':
            return self._parse_echo()
        elif tok.type == 'IF':
            return self._parse_if()
        elif tok.type == 'WHILE':
            return self._parse_while()
        elif tok.type == 'FUNC':
            return self._parse_func_def()
        elif tok.type == 'THINGY':
            return self._parse_assignment()
        elif tok.type == 'ID':
            if (self._position + 1 < len(self._tokens)
                    and self._tokens[self._position + 1].type == 'ASSIGN'):
                return self._parse_reassignment()
            return self._parse_func_call_or_error()
        elif tok.type == 'RETURN':
            return self._parse_return()
        else:
            raise SyntaxError(
                f"Unexpected token {tok.type} "
                f"on line {tok.line} "
                f"in {self._source_file}"
            )

    def _parse_facts(self) -> tuple:
        """
        Parse a 'facts' statement.

        Syntax:
            facts <expression>

        Returns:
            tuple: ('facts', expression_node, line_number)

        Raises:
            SyntaxError: If the expression is malformed.
        """
        tok = self._current_token
        self._eat("FACTS")
        expr_node = self._expr()
        return ("facts", expr_node, tok.line)


    def _parse_echo(self) -> tuple:
        """
        Parse a 'woah' (echo) statement.

        Syntax:
            woah <expression>

        Returns:
            tuple: ('woah', expression_node, line_number)

        Raises:
            SyntaxError: If the expression is malformed.
        """
        tok = self._current_token
        self._eat("ECHO")
        expr_node = self._expr()

        return ("woah", expr_node, tok.line)


    def _parse_if(self) -> tuple:
        """
        Parse a conditional 'maybe' (if/else) statement.

        Syntax:
            maybe <condition> {
                ...
            } okthen {
                ...
            }

        Returns:
            tuple: ('maybe', condition_expr, then_block, else_block_or_None, line_number)

        Raises:
            SyntaxError: If condition or blocks are malformed.
        """
        tok = self._current_token
        self._eat("IF")
        condition = self._comparison()
        then_block = self._block()

        elif_cases = []
        while self._current_token.type == 'ELIF':
            self._eat('ELIF')
            cond = self._comparison()
            block = self._block()
            elif_cases.append((cond, block))

        else_block = None
        if self._current_token.type == 'ELSE':
            self._eat('ELSE')
            else_block = self._block()

        tail = else_block
        for cond_node, block_node in reversed(elif_cases):
            cond_line = cond_node[-1] if isinstance(cond_node, tuple) else tok.line
            tail = ('maybe', cond_node, block_node, tail, cond_line)

        return ('maybe', condition, then_block, tail, tok.line)


    def _parse_while(self) -> tuple:
        """
        Parse a 'roundabout' (while) loop.

        Syntax:
            roundabout <condition> {
                ...
            }

        Returns:
            tuple: ('roundabout', condition_expr, block_node, line_number)

        Raises:
            SyntaxError: If the condition or block is invalid.
        """
        tok = self._current_token
        self._eat('WHILE')
        condition = self._comparison()
        body = self._block()
        return ('roundabout', condition, body, tok.line)


    def _parse_func_def(self) -> tuple:
        """
        Parse a function definition.

        Syntax:
            bitchin <name>(<param1>, <param2>, ...) {
                ...
            }

        Returns:
            tuple: ('func_def', function_name, [param_names], block_node, line_number)

        Raises:
            SyntaxError: If the syntax is malformed or parameters are invalid.
        """
        start_tok = self._current_token
        self._eat('FUNC')
        func_name = self._current_token.value
        self._eat('ID')
        self._eat('LPAREN')
        params = []
        if self._current_token.type != 'RPAREN':
            params.append(self._current_token.value)
            self._eat('ID')
            while self._current_token.type == 'COMMA':
                self._eat('COMMA')
                params.append(self._current_token.value)
                self._eat('ID')
        self._eat('RPAREN')
        body = self._block()
        return ('func_def', func_name, params, body, start_tok.line)


    def _parse_func_call_or_error(self) -> tuple:
        """
        Parse a function call or raise a syntax error.

        Syntax:
            <identifier>(<arg1>, <arg2>, ...)

        Returns:
            tuple: ('func_call', function_name, [arg_exprs], line_number)

        Raises:
            SyntaxError: If the call syntax is invalid or unexpected tokens follow.
        """
        tok = self._current_token
        func_name = tok.value
        self._eat('ID')
        if self._current_token.type == 'LPAREN':
            self._eat('LPAREN')
            args = []
            if self._current_token.type != 'RPAREN':
                args.append(self._expr())  # parse an expression argument
                while self._current_token.type == 'COMMA':
                    self._eat('COMMA')
                    args.append(self._expr())
            self._eat('RPAREN')
            return ('func_call', func_name, args, tok.line)
        else:
            # Could be a variable usage statement or error if not expected
            raise SyntaxError(
                f"Unexpected token '{tok.value}' after identifier {func_name} "
                f"on line {tok.line} in {self._source_file}"
            )


    def _parse_return(self) -> tuple:
        """
        Parse a 'gimme' return statement.

        Syntax:
            gimme <expression>

        Returns:
            tuple: ('return', expression_node, line_number)

        Raises:
            SyntaxError: If the return expression is malformed.
        """
        tok = self._current_token
        self._eat("RETURN")
        expr_node = self._expr()
        return ("return", expr_node, tok.line)


    def _parse_reassignment(self) -> tuple:
        """Parse reassignment of an existing variable.

        Syntax:
            <identifier> := <expression>
        """
        id_tok = self._current_token
        self._eat('ID')
        self._eat('ASSIGN')
        expr_node = self._expr()
        return ('assign', id_tok.value, expr_node, id_tok.line)


    def _parse_assignment(self) -> tuple:
        """
        Parse a 'thingy' variable assignment.

        Syntax:
            thingy <name> := <expression>

        Returns:
            tuple: ('assign', var_name, expr_node, line_number)

        Raises:
            SyntaxError: If the variable name, ':=' operator, or expression is missing or invalid.
        """
        self._eat('THINGY')
        id_tok = self._current_token
        if id_tok.type != 'ID':
            raise SyntaxError(
                f"Expected identifier after 'thingy' "
                f"on line {id_tok.line} "
                f"in {self._source_file}"
            )
        var_name = id_tok.value
        self._eat('ID')
        if self._current_token.type != 'ASSIGN':
            raise SyntaxError(
                f"Expected ':=' after variable name "
                f"on line {self._current_token.line} in {self._source_file}"
            )
        self._eat('ASSIGN')
        expr_node = self._expr()
        return ('assign', var_name, expr_node, id_tok.line)


    def parse(self):
        """
        Parse the full input into a list of statements.

        Returns:
            list: A list of statement AST nodes.
        """
        statements = []
        while self._current_token.type != 'EOF':
            while self._current_token.type == 'NEWLINE':
                self._eat('NEWLINE')
            if self._current_token.type == 'EOF':
                break
            statements.append(self._statement())
            while self._current_token.type == 'NEWLINE':
                self._eat('NEWLINE')
        return statements
