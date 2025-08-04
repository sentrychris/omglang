"""
Main parser entry point for OMGlang.

This module defines the `Parser` class, which coordinates the recursive
descent parsing process. The actual parsing routines are split across
`core.parser.expressions` and `core.parser.statements`.
"""

from . import expressions as _expr
from . import statements as _stmt


class Parser:
    """OMGlang parser."""

    def __init__(self, tokens: list, token_map_literals: dict[str, str], file: str):
        """
        Initialize the parser with a list of tokens.

        Parameters:
            tokens (list): A list of Token instances.
            token_map_literals (dict): A dict of tokens mapped to their literals.
            file (str): The name of the script.
        """
        self.tokens = tokens
        self.token_map = token_map_literals
        self.reverse_token_map = {v: k for k, v in self.token_map.items()}
        self.position = 0
        self.curr_token = self.tokens[self.position]
        self.source_file = file


    def eat(self, token_type: str) -> None:
        """
        Consume the current token if it matches the expected type.

        Parameters:
            token_type (str): The expected token type.

        Raises:
            SyntaxError: If the token does not match the expected type.
        """
        if self.curr_token.type == token_type:
            self.position += 1
            self.curr_token = self.tokens[self.position]
        else:
            expd_value = self.reverse_token_map.get(token_type, token_type)
            act_value = self.curr_token.value
            act_type = self.curr_token.type
            mapped_type = self.token_map.get(act_value, None)
            mapped_hint = f" (which maps to {mapped_type})" if mapped_type else ""

            raise SyntaxError(
                f"Expected token '{expd_value}' of type {token_type}, "
                f"but got value '{act_value}' of type {act_type}{mapped_hint} "
                f"on line {self.curr_token.line} in {self.source_file}"
            )


    # Expression wrappers
    def factor(self) -> tuple:
        """
        Parse a factor expression such as a literal, variable, or parenthesized group.
        """
        return _expr.parse_factor(self)

    def term(self) -> tuple:
        """
        Parse a term in an expression, typically involving multiplication or division.
        """
        return _expr.parse_term(self)

    def add_sub(self) -> tuple:
        """
        Parse an addition or subtraction expression.
        """
        return _expr.parse_add_sub(self)

    def shift(self) -> tuple:
        """
        Parse a bit-shift expression using left or right shift operators.
        """
        return _expr.parse_shift(self)

    def bitwise_and(self) -> tuple:
        """
        Parse a bitwise AND expression.
        """
        return _expr.parse_bitwise_and(self)

    def bitwise_xor(self) -> tuple:
        """
        Parse a bitwise XOR expression.
        """
        return _expr.parse_bitwise_xor(self)

    def bitwise_or(self) -> tuple:
        """
        Parse a bitwise OR expression.
        """
        return _expr.parse_bitwise_or(self)

    def comparison(self) -> tuple:
        """
        Parse a comparison expression using relational operators.
        """
        return _expr.parse_comparison(self)

    def logical_and(self) -> tuple:
        """
        Parse a logical AND expression.
        """
        return _expr.parse_logical_and(self)

    def expr(self) -> tuple:
        """
        Parse a full expression with arithmetic or logical operations.
        """
        return _expr.parse_expr(self)


    # Statement wrappers
    def block(self) -> tuple:
        """
        Parse a block of statements enclosed in delimiters.
        """
        return _stmt.parse_block(self)

    def statement(self) -> tuple:
        """
        Parse a single statement.
        """
        return _stmt.parse_statement(self)

    def parse_facts(self) -> tuple:
        """
        Parse a 'facts' statement.
        """
        return _stmt.parse_facts(self)

    def parse_emit(self) -> tuple:
        """
        Parse an 'emit' statement used for output.
        """
        return _stmt.parse_emit(self)

    def parse_if(self) -> tuple:
        """
        Parse an 'if' conditional statement.
        """
        return _stmt.parse_if(self)

    def parse_loop(self) -> tuple:
        """
        Parse a 'loop' statement.
        """
        return _stmt.parse_loop(self)

    def parse_break(self) -> tuple:
        """
        Parse a 'break' statement for loop termination.
        """
        return _stmt.parse_break(self)

    def parse_func_def(self) -> tuple:
        """
        Parse a function definition statement.
        """
        return _stmt.parse_func_def(self)

    def parse_return(self) -> tuple:
        """
        Parse a 'return' statement from within a function.
        """
        return _stmt.parse_return(self)

    def parse_reassignment(self) -> tuple:
        """
        Parse a variable reassignment statement.
        """
        return _stmt.parse_reassignment(self)

    def parse_assignment(self) -> tuple:
        """
        Parse a new variable assignment statement.
        """
        return _stmt.parse_assignment(self)


    def parse(self):
        """
        Parse the full input into a list of statements.
        """
        statements = []
        while self.curr_token.type != 'EOF':
            while self.curr_token.type == 'NEWLINE':
                self.eat('NEWLINE')
            if self.curr_token.type == 'EOF':
                break
            statements.append(self.statement())
            while self.curr_token.type == 'NEWLINE':
                self.eat('NEWLINE')
        return statements
