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
types (`UndefinedVariableException`, `UnknownOpException`) are raised on invalid references.

4. Control Flow
Control constructs include:
- `if`/'else' (if/else): executes conditional blocks based on boolean evaluation.
- `loop` (while): repeatedly evaluates a block while a condition holds.
- `block`: executes a nested sequence of statements.

5. Header Validation
Before evaluation, the interpreter checks for a required script header (`;;;omg`). If not found,
execution is aborted with a descriptive runtime error.

6. Error Handling
Runtime errors during interpretation—, such as undefined variables, unknown operations, or
malformed AST nodes, are surfaced as typed exceptions with line numbers and file context.
"""
from core.exceptions import (
    UndefinedVariableException,
    UnknownOpException,
    BreakLoop,
    ReturnControlFlow,
)
from core.operations import Op


class Interpreter:
    """
    Tree-walk interpreter for OMGlang.
    """
    def __init__(self, file: str):
        """
        Initialize the interpreter.
        """
        self.vars = {}
        self.global_vars = self.vars
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
            f"OMG script missing required header ';;;omg' "
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
            f"OMG script missing required header ';;;omg'\n"
            f"in {self.file}"
        )

    def _format_expr(self, node) -> str:
        """
        Convert AST back to a readable string for debugging.
        """
        op = node[0]
        match op:
            case Op.EQ:
                return (
                    f"({self._format_expr(node[1])} == "
                    f"{self._format_expr(node[2])})"
                )
            case 'ident':
                return node[1]
            case 'number' | 'bool' | 'int':
                return str(node[1])
            case 'index':
                args = node[2]
                return f"{node[1]}[{args[1]}]"
            case 'func_call':
                fname, args, _ = node[1], node[2], node[3]
                return f"{fname}({', '.join(self._format_expr(arg) for arg in args)})"
            case _:
                name = op if isinstance(op, str) else op.value
                return f"<expr {name}>"

    def eval_expr(self, node):
        """
        Recursively evaluate an expression node and return its computed value.

        Parameters:
            node (tuple): An expression node, structured as a tuple.
                        The first element is the operation type (e.g., 'add', 'ident'),
                        followed by operands and the line number for error reporting.

        Returns:
            The evaluated result of the expression.

        Raises:
            UndefinedVariableException: If a variable is referenced that has not been defined.
            UnknownOpException: If an unrecognized binary operator is encountered.
            RuntimeError: If the expression node format is invalid or improperly structured.
        """
        if isinstance(node, tuple):
            op = node[0]
            line = node[-1]

            # Literals
            if op == 'number':
                return node[1]
            elif op == 'string':
                return node[1]
            elif op == 'bool':
                return node[1]
            elif op == 'list':
                _, elements, _ = node
                return [self.eval_expr(elem) for elem in elements]

            # Variables
            elif op == 'ident':
                varname = node[1]
                if varname in self.vars:
                    return self.vars[varname]
                if varname in self.global_vars:
                    return self.global_vars[varname]
                raise UndefinedVariableException(varname, line, self.file)

            # Indexes
            elif op == 'index':
                _, target_name, index_expr, _ = node

                if target_name in self.vars:
                    target = self.vars[target_name]
                elif target_name in self.global_vars:
                    target = self.global_vars[target_name]
                else:
                    raise UndefinedVariableException(target_name, line, self.file)
                index = self.eval_expr(index_expr)

                if isinstance(target, list):
                    if not 0 <= index < len(target):
                        raise RuntimeError(
                            f"List index out of bounds!\n"
                            f"{self._format_expr(node)}\n"
                            f"On line {line} in {self.file}"
                        )
                    return target[index]
                elif isinstance(target, str):
                    if not 0 <= index < len(target):
                        raise RuntimeError(
                            f"String index out of bounds!\n"
                            f"{self._format_expr(node)}\n"
                            f"On line {line} in {self.file}"
                        )
                    return target[index]
                else:
                    raise TypeError(
                        f"{target_name} is not indexable!"
                        f"{self._format_expr(node)}\n"
                        f"On line {line} in {self.file}"
                    )

            # Index slicing
            elif op == 'slice':
                _, target_name, start_expr, end_expr, _ = node

                if target_name in self.vars:
                    target = self.vars[target_name]
                elif target_name in self.global_vars:
                    target = self.global_vars[target_name]
                else:
                    raise UndefinedVariableException(target_name, line, self.file)
                start = self.eval_expr(start_expr)
                end = self.eval_expr(end_expr) if end_expr is not None else None

                if isinstance(target, (list, str)):
                    return target[start:end]
                else:
                    raise TypeError(
                        f"{target_name} is not sliceable!\n"
                        f"{self._format_expr(node)}\n"
                        f"On line {line} in {self.file}"
                    )

            # Binary operations
            elif op in (
                Op.ADD,
                Op.SUB,
                Op.MUL,
                Op.MOD,
                Op.DIV,
                Op.AND_BITS,
                Op.OR_BITS,
                Op.XOR_BITS,
                Op.SHL,
                Op.SHR,
                Op.EQ,
                Op.NE,
                Op.GT,
                Op.LT,
                Op.GE,
                Op.LE,
                Op.AND,
            ):
                lhs = self.eval_expr(node[1])
                if op == Op.AND:
                    if not bool(lhs):
                        return False
                    rhs = self.eval_expr(node[2])
                    return bool(rhs)
                rhs = self.eval_expr(node[2])
                match op:
                    # Arithmetic
                    case Op.ADD:
                        if isinstance(lhs, str) or isinstance(rhs, str):
                            term = str(lhs) + str(rhs)
                        else:
                            term = lhs + rhs
                    case Op.SUB:
                        term = lhs - rhs
                    case Op.MUL:
                        term = lhs * rhs
                    case Op.MOD:
                        term = lhs % rhs
                    case Op.DIV:
                        term = lhs // rhs
                    # Bitwise
                    case Op.AND_BITS:
                        term = lhs & rhs
                    case Op.OR_BITS:
                        term = lhs | rhs
                    case Op.XOR_BITS:
                        term = lhs ^ rhs
                    case Op.SHL:
                        term = lhs << rhs
                    case Op.SHR:
                        term = lhs >> rhs
                    # Comparison
                    case Op.EQ:
                        term = lhs == rhs
                    case Op.NE:
                        term = lhs != rhs
                    case Op.GT:
                        term = lhs > rhs
                    case Op.LT:
                        term = lhs < rhs
                    case Op.GE:
                        term = lhs >= rhs
                    case Op.LE:
                        term = lhs <= rhs
                    case _:
                        raise UnknownOpException(
                            f"Unknown binary operator '{op}'"
                        )
                return term

            # Unary operator
            elif op == 'unary':
                operator = node[1]
                operand = self.eval_expr(node[2])
                match operator:
                    case Op.NOT_BITS:
                        if not isinstance(operand, int):
                            raise TypeError(
                                f"Bitwise NOT (~) requires an integer operand "
                                f"{self._format_expr(node)}\n"
                                f"On line {line} in {self.file}"
                            )
                        return ~operand
                    case Op.ADD:
                        if not isinstance(operand, int):
                            raise TypeError(
                                f"Unary plus (+) requires a numeric operand "
                                f"{self._format_expr(node)}\n"
                                f"On line {line} in {self.file}"
                            )
                        return +operand
                    case Op.SUB:
                        if not isinstance(operand, int):
                            raise TypeError(
                                f"Unary minus (-) requires a numeric operand "
                                f"{self._format_expr(node)}\n"
                                f"On line {line} in {self.file}"
                            )
                        return -operand
                    case _:
                        raise UnknownOpException(
                            f"Unknown unary operator '{operator}'!"
                            f"{self._format_expr(node)}\n"
                            f"On line {line} in {self.file}"
                        )

            # Function calls
            elif op == 'func_call':
                _, func_name, args_nodes, line = node
                args = [self.eval_expr(arg) for arg in args_nodes]

                # Built-in functions
                if func_name == 'chr':
                    if len(args) != 1 or not isinstance(args[0], int):
                        raise TypeError(
                            f"chr() expects one integer argument!\n"
                            f"on line {line} in {self.file}"
                        )
                    return chr(args[0])

                if func_name == 'ascii':
                    if len(args) != 1 or not isinstance(args[0], str) or len(args[0]) != 1:
                        raise TypeError(
                            f"ascii() expects a single-character string argument!\n"
                            f"on line {line} in {self.file}"
                        )
                    return ord(args[0])

                if func_name == 'hex':
                    if len(args) != 1 or not isinstance(args[0], int):
                        raise TypeError(
                            f"hex() expects one integer argument!\n"
                            f"on line {line} in {self.file}"
                        )
                    return str(hex(args[0])[2:]).upper()

                if func_name == 'binary':
                    if (
                        len(args) not in (1, 2)
                        or not isinstance(args[0], int)
                        or (len(args) == 2 and not isinstance(args[1], int))
                    ):
                        raise TypeError(
                            f"binary() expects an integer and optional width integer!\n"
                            f"on line {line} in {self.file}"
                        )
                    n = args[0]
                    if len(args) == 1:
                        return ('-' + bin(abs(n))[2:]) if n < 0 else bin(n)[2:]
                    width = args[1]
                    if width <= 0:
                        raise ValueError(
                            f"binary() width must be positive!\n"
                            f"on line {line} in {self.file}"
                        )
                    mask = (1 << width) - 1
                    return format(n & mask, f'0{width}b')

                if func_name == 'length':
                    if len(args) != 1:
                        raise TypeError(
                            f"length() expects one argument on line!\n"
                            f"on line {line} in {self.file}"
                        )
                    arg = args[0]
                    if not isinstance(arg, (list, str)):
                        raise TypeError(
                            f"length() only works on lists or strings!\n"
                            f"on line {line} in {self.file}"
                        )
                    return len(arg)

                # User-defined functions
                if func_name not in self.functions:
                    raise TypeError(
                        f"Undefined function '{func_name}'!\n"
                        f"{self._format_expr(node)}\n"
                        f"On line {line} in {self.file}"
                    )

                params, body = self.functions[func_name]

                if len(args) != len(params):
                    raise TypeError(
                        f"Function '{func_name}' expects {len(params)} arguments "
                        f"{self._format_expr(node)}\n"
                        f"On line {line} in {self.file}"
                    )

                saved_vars = self.vars
                self.vars = {}
                self.vars.update(dict(zip(params, args)))

                try:
                    if body[0] == "block":
                        self.execute(body[1])
                    else:
                        self.execute([body])
                    result = None
                except ReturnControlFlow as ret:
                    result = ret.value

                self.vars = saved_vars
                return result

        raise RuntimeError(f"Invalid expression node: {node}")

    def execute(self, statements: list):
        """
        Executes a list of statements.

        Parameters:
            statements (list):
                A list of ('decl' | 'assign' | 'emit' | 'if' | 'block' | 'loop', ...) tuples.

        Raises:
            Exception: For unknown statement types.
        """
        for stmt in statements:
            kind = stmt[0]
            line = stmt[-1]

            if kind == 'decl':
                _, var_name, expr_node, _ = stmt
                if var_name in self.vars:
                    raise RuntimeError(
                        f"Variable '{var_name}' already declared on line {line} in {self.file}"
                    )
                value = self.eval_expr(expr_node)
                self.vars[var_name] = value

            elif kind == 'assign':
                _, var_name, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                if var_name in self.vars:
                    self.vars[var_name] = value
                elif var_name in self.global_vars:
                    self.global_vars[var_name] = value
                else:
                    raise UndefinedVariableException(var_name, line, self.file)

            elif kind == 'emit':
                _, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                print(value)

            elif kind == 'facts':
                _, expr_node, line = stmt
                value = self.eval_expr(expr_node)
                if not value:
                    raise AssertionError(
                        f"Assertion failed on line {line}: {self._format_expr(expr_node)}"
                    )

            elif kind == 'if':
                _, cond_node, then_block, else_block, _ = stmt
                if self.eval_expr(cond_node):
                    self.execute([then_block])
                elif else_block:
                    self.execute([else_block])

            elif kind == 'block':
                _, block_statements, _ = stmt
                self.execute(block_statements)

            elif kind == 'loop':
                _, cond_node, block_node, _ = stmt
                try:
                    while self.eval_expr(cond_node):
                        try:
                            self.execute([block_node])
                        except BreakLoop:
                            break
                except BreakLoop:
                    pass

            elif kind == 'break':
                raise BreakLoop()

            elif kind == 'func_def':
                _, name, params, body, _ = stmt
                self.functions[name] = (params, body)

            elif kind == 'return':
                _, expr_node, _ = stmt
                value = self.eval_expr(expr_node)
                raise ReturnControlFlow(value)

            elif kind == 'expr_stmt':
                _, expr_node, _ = stmt
                self.eval_expr(expr_node)

            else:
                raise TypeError(
                    f"Unknown statement type: {kind} "
                    f"{self._format_expr(expr_node)}"
                    f"On line {line} in {self.file}"
                )
