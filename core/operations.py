"""Shared definitions for AST operation identifiers.

This module centralizes the string constants used by the parser and
interpreter to label nodes in the abstract syntax tree.  Keeping them in one
place prevents the two components from drifting apart when new operations are
added or existing ones are renamed.
"""

from enum import Enum


class Op(str, Enum):
    """
    Enumeration of supported AST operation names.
    """

    # Arithmetic
    ADD = "add"
    SUB = "sub"
    MUL = "mul"
    DIV = "div"
    MOD = "mod"

    # Bitwise
    AND_BITS = "and_bits"
    OR_BITS = "or_bits"
    XOR_BITS = "xor_bits"
    SHL = "shl"
    SHR = "shr"

    # Comparison
    EQ = "eq"
    NE = "ne"
    GT = "gt"
    LT = "lt"
    GE = "ge"
    LE = "le"

    # Unary bitwise
    NOT_BITS = "not_bits"

    # Boolean
    AND = "and"
    OR = "or"

    def __str__(self) -> str:  # pragma: no cover - trivial
        """
        Return the underlying string value for nicer debug output.
        """
        return self.value


__all__ = ["Op"]
