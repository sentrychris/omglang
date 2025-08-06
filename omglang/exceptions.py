"""Errors.


File: exceptions.py
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © 2025 Chris Rowles. All rights reserved.
Version: 0.1.0
License: MIT
"""


class UndefinedVariableException(Exception):
    """
    Error for undefined variables.
    """
    def __init__(self, varname, line=None, file=None):
        self.varname = varname
        self.line = line
        message = f"Undefined variable '{varname}'"
        if line is not None:
            message += f" on line {line}"
        if file is not None:
            message += f" in {file}"
        super().__init__(message)


class UnknownOpException(Exception):
    """
    Error for unknown operations.
    """
    def __init__(self, op, line=None, file=None):
        self.op = op
        self.line = line
        message = f"Unknown operation '{op}'"
        if line is not None:
            message += f" on line {line}"
        if file is not None:
            message += f" in {file}"
        super().__init__(message)


class ReturnControlFlow(Exception):
    """
    Control flow handling for return statements.
    """
    def __init__(self, value):
        self.value = value


class BreakLoop(Exception):
    """
    Control flow handling for return statements.
    """
    pass
