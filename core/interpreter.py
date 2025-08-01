"""
Interpreter.
"""

import sys

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

class Interpreter:
    """
    A simple expression interpreter supporting variables and arithmetic.
    """
    def __init__(self, file: str):
        """
        Initialize the interpreter with an empty variable environment.
        """
        self.vars = {}
        self.file = file


    def check_header(self, source_code: str):
        """
        Ensure the source code starts with ';;;crsi' on the first non-empty line.
        
        Raises:
            RuntimeError: If the header is missing or incorrect.
        """
        for line in source_code.splitlines():
            if line.strip() == '':
                continue
            if line.strip() == ';;;crsi':
                return
        raise RuntimeError(
            f"CRS script missing required header ';;;crsi'\n"
            f"in {self.file}"
        )


    def strip_header(self, source_code: str) -> str:
        """
        Strip the header from the source code before evaluation.

        Raises:
            RuntimeError if the header is missing.
        """
        lines = source_code.splitlines()
        for i, line in enumerate(lines):
            if line.strip() == ';;;crsi':
                return '\n'.join(lines[i + 1:])
        raise RuntimeError("" \
            f"CRS script missing required header ';;;crsi'\n"
            f"in {self.file}"
        )


    def eval_expr(self, node):
        """
        Evaluate AST node.
        """
        if isinstance(node, tuple):
            op = node[0]
            line = node[-1]

            if op == 'number':
                return node[1]
            elif op == 'string':
                return node[1]
            elif op == 'var':
                varname = node[1]
                if varname in self.vars:
                    return self.vars[varname]
                raise UndefinedVariableError(varname, line, self.file)
            elif op in ('add', 'sub', 'mul', 'div', 'eq', 'gt', 'lt', 'ge', 'le'):
                lhs = self.eval_expr(node[1])
                rhs = self.eval_expr(node[2])
                match op:
                    case 'add': return lhs + rhs
                    case 'sub': return lhs - rhs
                    case 'mul': return lhs * rhs
                    case 'div': return lhs / rhs
                    case 'eq': return lhs == rhs
                    case 'gt': return lhs > rhs
                    case 'lt': return lhs < rhs
                    case 'ge': return lhs >= rhs
                    case 'le': return lhs <= rhs
            else:
                raise UnknownOperationError(op, line, self.file)
        raise TypeError(f"Invalid expression node: {node}")


    def execute(self, statements: list):
        """
        Execute a list of statements.

        Parameters:
            statements (list): A list of ('assign' | 'cout', ...) tuples.

        Raises:
            Exception: For unknown statement types.
        """
        for stmt in statements:
            kind = stmt[0]
            line = stmt[-1]

            if kind == 'assign':
                _, var_name, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                self.vars[var_name] = value

            elif kind == 'cout':
                _, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                print(value)

            elif kind == 'if':
                _, cond_node, then_block, else_block, _ = stmt
                if self.eval_expr(cond_node):
                    self.execute([then_block])
                elif else_block:
                    self.execute([else_block])

            elif kind == 'block':
                _, block_statements, _ = stmt
                self.execute(block_statements)

            else:
                raise TypeError(
                    f"Unknown statement type: {kind} "
                    f"on line {line}"
                    f"in {self.file}"
                )
