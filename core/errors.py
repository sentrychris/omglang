class UndefinedVariableError(Exception):
    def __init__(self, varname, line=None, file=None):
        self.varname = varname
        self.line = line
        message = f"Undefined variable '{varname}'"
        if line is not None:
            message += f" on line {line}"
        if file is not None:
            message += f" in {file}"
        super().__init__(message)


class UnknownOperationError(Exception):
    def __init__(self, op, line=None, file=None):
        self.op = op
        self.line = line
        message = f"Unknown operation '{op}'"
        if line is not None:
            message += f" on line {line}"
        if file is not None:
            message += f" in {file}"
        super().__init__(message)