"""
Interpreter.

This is a tree-walk interpreter for evaluating AST nodes produced by the parser. It supports 
arithmetic, variables, function definitions and calls, conditionals, loops, and output statements.

1. Execution Model
The interpreter evaluates an abstract syntax tree (AST) in a top-down, recursive manner. 
Statements are executed via the `execute()` method, and expressions are evaluated using 
`eval_expr()`. Both methods operate over structured tuples representing nodes in the AST.

2. Environment
The interpreter maintains two dictionaries:
    - `vars`: a variable environment (scope) for storing and retrieving user-defined values.
    - `functions`: a function table for storing parameter lists and function bodies by name.
Function calls temporarily override the variable environment to simulate local scope, and restore 
it afterward to preserve state between calls.

3. Expression Evaluation
Expression nodes (e.g. arithmetic operations, comparisons, literals, variable references, and 
function calls) are evaluated recursively. Basic type checking is enforced, and custom error 
types (`UndefinedVariableError`, `UnknownOperationError`) are raised on invalid references.

4. Control Flow
Control constructs include:
- `maybe`/'okthen' (if/else): executes conditional blocks based on boolean evaluation.
- `hamsterwheel` (while): repeatedly evaluates a block while a condition holds.
- `block`: executes a nested sequence of statements.

5. Header Validation
Before evaluation, the interpreter checks for a required script header (`;;;omg`). If not found, 
execution is aborted with a descriptive runtime error.

6. Error Handling
Runtime errors during interpretation—, such as undefined variables, unknown operations, or 
malformed AST nodes, are surfaced as typed exceptions with line numbers and file context.
"""
from core.errors import UndefinedVariableError, UnknownOperationError

class Interpreter:
    """
    Tree-walk interpreter for OMGlang.
    """
    def __init__(self, file: str):
        """
        Initialize the interpreter.
        """
        self.vars = {}
        self.functions = {}
        self.file = file


    def check_header(self, source_code: str):
        """
        Ensure the source code starts with header on the first non-empty line.
        
        Raises:
            RuntimeError: If the header is missing or incorrect.
        """
        for line in source_code.splitlines():
            if line.strip() == '':
                continue
            if line.strip() == ';;;omg':
                return
        raise RuntimeError(
            f"CRS script missing required header ';;;omg'\n"
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
            if line.strip() == ';;;omg':
                return '\n'.join(lines[i + 1:])
        raise RuntimeError(
            f"CRS script missing required header ';;;omg'\n"
            f"in {self.file}"
        )


    def eval_expr(self, node):
        """
        Recursively evaluate an expression node and return its computed value.

        Parameters:
            node (tuple): An expression node, structured as a tuple.
                        The first element is the operation type (e.g., 'add', 'thingy'),
                        followed by operands and the line number for error reporting.

        Returns:
            The evaluated result of the expression.

        Raises:
            UndefinedVariableError: If a variable is referenced that has not been defined.
            UnknownOperationError: If an unrecognized binary operator is encountered.
            RuntimeError: If the expression node format is invalid or improperly structured.
        """
        if isinstance(node, tuple):
            op = node[0]
            line = node[-1]

            if op == 'number':
                return node[1]
            elif op == 'string':
                return node[1]
            elif op == 'thingy':
                varname = node[1]
                if varname in self.vars:
                    return self.vars[varname]
                raise UndefinedVariableError(varname, line, self.file)
            elif op in ('add', 'sub', 'mul', 'mod', 'div', 'eq', 'gt', 'lt', 'ge', 'le'):
                lhs = self.eval_expr(node[1])
                rhs = self.eval_expr(node[2])
                match op:
                    case 'add':
                        if isinstance(lhs, str) or isinstance(rhs, str):
                            term = str(lhs) + str(rhs)
                        else:
                            term = lhs + rhs
                    case 'sub': term = lhs - rhs
                    case 'mul': term = lhs * rhs
                    case 'mod': term = lhs % rhs
                    case 'div': term = lhs / rhs
                    case 'eq':  term = lhs == rhs
                    case 'gt':  term = lhs > rhs
                    case 'lt':  term = lhs < rhs
                    case 'ge':  term = lhs >= rhs
                    case 'le':  term = lhs <= rhs
                    case _:
                        raise UnknownOperationError(f"Unknown binary operator '{op}'")
                return term

        raise RuntimeError(f"Invalid expression node: {node}")


    def execute(self, statements: list):
        """
        Executes a list of statements.

        Parameters:
            statements (list):
                A list of ('assign' | 'saywhat' | 'maybe' | 'block' | 'hamsterwheel', ...) tuples.

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


            elif kind == 'saywhat':
                _, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                print(value)


            elif kind == 'facts':
                _, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                assert value


            elif kind == 'maybe':
                _, cond_node, then_block, else_block, _ = stmt
                if self.eval_expr(cond_node):
                    self.execute([then_block])
                elif else_block:
                    self.execute([else_block])


            elif kind == 'block':
                _, block_statements, _ = stmt
                self.execute(block_statements)


            elif kind == 'hamsterwheel':
                _, cond_node, block_node, _ = stmt
                while self.eval_expr(cond_node):
                    self.execute([block_node])


            elif kind == 'func_def':
                _, name, params, body, _ = stmt
                self.functions[name] = (params, body)
                # print(self.functions)


            elif kind == 'func_call':
                _, func_name, args_nodes, _ = stmt
                # Evaluate each argument expression
                args = [self.eval_expr(arg) for arg in args_nodes]

                # Lookup function by name
                if func_name not in self.functions:
                    raise NameError(
                        f"Undefined function '{func_name}' on line {line} in {self.file}"
                    )

                params, body = self.functions[func_name]

                if len(args) != len(params):
                    raise TypeError(
                        f"Function '{func_name}' expects {len(params)} arguments "
                        f"but got {len(args)} on line {line} in {self.file}"
                    )

                # Create a new local scope for function execution
                saved_vars = self.vars.copy()
                self.vars.update(dict(zip(params, args)))

                # Execute the function body (which is presumably a block node)
                self.execute([body])

                # Restore the previous variable scope
                self.vars = saved_vars


            else:
                raise TypeError(
                    f"Unknown statement type: {kind} "
                    f"on line {line} "
                    f"in {self.file}"
                )
