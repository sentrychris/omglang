"""
Parser.
"""
class Parser:
    """
    A simple recursive descent parser for expressions and statements.
    """
    def __init__(self, tokens: list, file: str):
        """
        Initialize the parser with a list of tokens.

        Parameters:
            tokens (list): A list of Token instances.
            file (str): The name of the script.
        """
        self.tokens = tokens
        self.pos = 0
        self.current_token = self.tokens[self.pos]
        self.file = file


    def eat(self, token_type: str):
        """
        Consume the current token if it matches the expected type.

        Parameters:
            token_type (str): The expected token type.

        Raises:
            SyntaxError: If the token does not match the expected type.
        """
        if self.current_token.type == token_type:
            self.pos += 1
            self.current_token = self.tokens[self.pos]
        else:
            raise SyntaxError(
                f"Expected {token_type}, "
                f"got {self.current_token.type} "
                f"on line {self.current_token.line} "
                f"in {self.file}"
            )


    def factor(self):
        """
        Parse a factor (number, string, variable, or parenthesized expression).

        Returns:
            An AST node representing the factor.
        """
        tok = self.current_token
        if tok.type == 'NUMBER':
            self.eat('NUMBER')
            return ('number', tok.value, tok.line)
        elif tok.type == 'STRING':
            self.eat('STRING')
            return ('string', tok.value, tok.line)
        elif tok.type == 'ID':
            self.eat('ID')
            return ('var', tok.value, tok.line)
        elif tok.type == 'LPAREN':
            self.eat('LPAREN')
            node = self.expr()
            self.eat('RPAREN')
            return node
        else:
            raise SyntaxError(
                f"Unexpected token {tok.type} "
                f"on line {tok.line} "
                f"in {self.file}"
            )


    def term(self):
        """
        Parse a term (factor optionally followed by * or /).

        Returns:
            An AST node representing the term.
        """
        result = self.factor()
        while self.current_token.type in ('MUL', 'DIV'):
            op_tok = self.current_token
            self.eat(op_tok.type)
            op_map = {'MUL': 'mul', 'DIV': 'div'}
            result = (op_map[op_tok.type], result, self.factor(), op_tok.line)
        return result


    def expr(self):
        """
        Parse an expression (term optionally followed by + or -).

        Returns:
            An AST node representing the expression.
        """
        result = self.term()
        while self.current_token.type in ('PLUS', 'MINUS'):
            op_tok = self.current_token
            self.eat(op_tok.type)
            op_map = {'PLUS': 'add', 'MINUS': 'sub'}
            result = (op_map[op_tok.type], result, self.term(), op_tok.line)
        return result


    def comparison(self):
        """
        Parse a comparison expression (e.g., ==, <, >, <=, >=).

        Returns:
            An AST node representing the comparison.
        """
        result = self.expr()
        while self.current_token.type in ('EQ', 'GT', 'LT', 'GE', 'LE'):
            op_tok = self.current_token
            self.eat(op_tok.type)
            op_map = {'EQ': 'eq', 'GT': 'gt', 'LT': 'lt', 'GE': 'ge', 'LE': 'le'}
            result = (op_map[op_tok.type], result, self.expr(), op_tok.line)
        return result


    def block(self):
        """
        Parse a block of statements enclosed in braces.

        Returns:
            A ('block', list_of_statements, line_number) AST node.
        """
        tok = self.current_token
        self.eat('LBRACE')
        statements = []
        while self.current_token.type != 'RBRACE':
            while self.current_token.type == 'NEWLINE':
                self.eat('NEWLINE')
            statements.append(self.statement())
            while self.current_token.type == 'NEWLINE':
                self.eat('NEWLINE')
        self.eat('RBRACE')
        return ('block', statements, tok.line)


    def statement(self):
        """
        Parse a single statement (cout, var assignment, maybe, or while).

        Returns:
            A tuple representing the statement AST node.

        Raises:
            SyntaxError: If the syntax is invalid or unexpected.
        """
        tok = self.current_token

        if tok.type == 'COUT':
            self.eat('COUT')
            self.eat('ARROW')
            expr_node = self.expr()
            return ('cout', expr_node, tok.line)

        elif tok.type == 'IF':
            self.eat('IF')
            condition = self.comparison()
            then_block = self.block()
            else_block = None
            if self.current_token.type == 'ELSE':
                self.eat('ELSE')
                else_block = self.block()
            return ('maybe', condition, then_block, else_block, tok.line)

        elif tok.type == 'WHILE':
            self.eat('WHILE')
            condition = self.comparison()
            body = self.block()
            return ('while', condition, body, tok.line)

        elif tok.type == 'VAR':
            self.eat('VAR')
            id_tok = self.current_token
            if id_tok.type != 'ID':
                raise SyntaxError(
                    f"Expected identifier after 'var' "
                    f"on line {id_tok.line} "
                    f"in {self.file}"
                )
            var_name = id_tok.value
            self.eat('ID')
            if self.current_token.type != 'ASSIGN':
                raise SyntaxError(
                    f"Expected ':=' after variable name "
                    f"on line {self.current_token.line} "
                    f"in {self.file}"
                )
            self.eat('ASSIGN')
            expr_node = self.expr()
            return ('assign', var_name, expr_node, id_tok.line)

        raise SyntaxError(
            f"Unexpected token {tok.type} "
            f"on line {tok.line} "
            f"in {self.file}"
        )


    def parse(self):
        """
        Parse the full input into a list of statements.

        Returns:
            list: A list of statement AST nodes.
        """
        statements = []
        while self.current_token.type != 'EOF':
            while self.current_token.type == 'NEWLINE':
                self.eat('NEWLINE')
            if self.current_token.type == 'EOF':
                break
            statements.append(self.statement())
            while self.current_token.type == 'NEWLINE':
                self.eat('NEWLINE')
        return statements
