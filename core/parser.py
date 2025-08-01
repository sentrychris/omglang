"""
Parser.
"""
class Parser:
    """
    A simple recursive descent parser for expressions and statements.
    """
    def __init__(self, tokens):
        """
        Initialize the parser with a list of tokens.

        Parameters:
            tokens (list): A list of Token instances.
        """
        self.tokens = tokens
        self.pos = 0
        self.current_token = self.tokens[self.pos]

    def eat(self, token_type):
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
            raise SyntaxError(f'Expected token {token_type}, got {self.current_token}')

    def factor(self):
        """
        Parse a factor (number, string, variable, or parenthesized expression).

        Returns:
            An AST node representing the factor.
        """
        tok = self.current_token
        if tok.type == 'NUMBER':
            self.eat('NUMBER')
            return tok.value
        elif tok.type == 'STRING':
            self.eat('STRING')
            return tok.value
        elif tok.type == 'ID':
            self.eat('ID')
            return ('var', tok.value)
        elif tok.type == 'LPAREN':
            self.eat('LPAREN')
            node = self.expr()
            self.eat('RPAREN')
            return node
        else:
            raise SyntaxError(f'Unexpected token {tok}')

    def term(self):
        """
        Parse a term (factor optionally followed by * or /).

        Returns:
            An AST node representing the term.
        """
        result = self.factor()
        while self.current_token.type in ('MUL', 'DIV'):
            op = self.current_token.type
            self.eat(op)
            op_map = {'MUL': 'mul', 'DIV': 'div'}
            result = (op_map[op], result, self.factor())
        return result

    def expr(self):
        """
        Parse an expression (term optionally followed by + or -).

        Returns:
            An AST node representing the expression.
        """
        result = self.term()
        while self.current_token.type in ('PLUS', 'MINUS'):
            op = self.current_token.type
            self.eat(op)
            op_map = {'PLUS': 'add', 'MINUS': 'sub'}
            result = (op_map[op], result, self.term())
        return result

    def statement(self):
        """
        Parse a single statement (cout or var assignment).

        Returns:
            A tuple representing the statement AST node.

        Raises:
            SyntaxError: If the syntax is invalid or unexpected.
        """
        if self.current_token.type == 'COUT':
            self.eat('COUT')
            self.eat('ARROW')
            expr_node = self.expr()
            return ('cout', expr_node)

        if self.current_token.type == 'VAR':
            self.eat('VAR')
            if self.current_token.type != 'ID':
                raise SyntaxError("Expected identifier after 'var'")
            var_name = self.current_token.value
            self.eat('ID')
            if self.current_token.type == 'ASSIGN':
                self.eat('ASSIGN')
                expr_node = self.expr()
                return ('assign', var_name, expr_node)
            else:
                raise SyntaxError("Expected ':=' after variable name")

        raise SyntaxError(f'Unexpected token {self.current_token.type}')

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
            stmt = self.statement()
            statements.append(stmt)
            while self.current_token.type == 'NEWLINE':
                self.eat('NEWLINE')
        return statements

