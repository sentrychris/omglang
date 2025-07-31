class Parser:
    def __init__(self, tokens):
        self.tokens = tokens
        self.pos = 0
        self.current_token = self.tokens[self.pos]

    def eat(self, token_type):
        if self.current_token.type == token_type:
            self.pos += 1
            self.current_token = self.tokens[self.pos]
        else:
            raise SyntaxError(f'Expected token {token_type}, got {self.current_token}')

    def factor(self):
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
        result = self.factor()
        while self.current_token.type in ('MUL', 'DIV'):
            op = self.current_token.type
            self.eat(op)
            op_map = {'MUL': 'mul', 'DIV': 'div'}
            result = (op_map[op], result, self.factor())
        return result

    def expr(self):
        result = self.term()
        while self.current_token.type in ('PLUS', 'MINUS'):
            op = self.current_token.type
            self.eat(op)
            op_map = {'PLUS': 'add', 'MINUS': 'sub'}
            result = (op_map[op], result, self.term())
        return result

    def statement(self):
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

