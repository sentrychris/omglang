"""Main parser entry point for OMGlang.

This module defines the :class:`Parser` class, which coordinates the
recursive descent parsing process. The actual parsing routines are split
across :mod:`core.parser.expressions` and :mod:`core.parser.statements`.
"""

from . import expressions as _expr
from . import statements as _stmt


class Parser:
    """OMGlang parser."""

    def __init__(self, tokens: list, token_map_literals: dict[str, str], file: str):
        """Initialize the parser with a list of tokens.

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
        """Consume the current token if it matches the expected type.

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

    # Expression wrappers
    def _factor(self) -> tuple:
        return _expr.parse_factor(self)

    def _term(self) -> tuple:
        return _expr.parse_term(self)

    def _bitwise_or(self) -> tuple:
        return _expr.parse_bitwise_or(self)

    def _bitwise_xor(self) -> tuple:
        return _expr.parse_bitwise_xor(self)

    def _bitwise_and(self) -> tuple:
        return _expr.parse_bitwise_and(self)

    def _shift(self) -> tuple:
        return _expr.parse_shift(self)

    def _add_sub(self) -> tuple:
        return _expr.parse_add_sub(self)

    def _expr(self) -> tuple:
        return _expr.parse_expr(self)

    def _comparison(self) -> tuple:
        return _expr.parse_comparison(self)

    # Statement wrappers
    def _block(self) -> tuple:
        return _stmt.parse_block(self)

    def _statement(self) -> tuple:
        return _stmt.parse_statement(self)

    def _parse_facts(self) -> tuple:
        return _stmt.parse_facts(self)

    def _parse_echo(self) -> tuple:
        return _stmt.parse_echo(self)

    def _parse_if(self) -> tuple:
        return _stmt.parse_if(self)

    def _parse_while(self) -> tuple:
        return _stmt.parse_while(self)

    def _parse_break(self) -> tuple:
        return _stmt.parse_break(self)

    def _parse_func_def(self) -> tuple:
        return _stmt.parse_func_def(self)

    def _parse_return(self) -> tuple:
        return _stmt.parse_return(self)

    def _parse_reassignment(self) -> tuple:
        return _stmt.parse_reassignment(self)

    def _parse_assignment(self) -> tuple:
        return _stmt.parse_assignment(self)

    def parse(self):
        """Parse the full input into a list of statements."""
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
