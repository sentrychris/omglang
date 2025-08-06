"""Interpreter.

This is a tree-walk interpreter for evaluating AST nodes produced by the parser. It supports
arithmetic, variables, function definitions and calls, conditionals, loops, and output statements.

1. Execution Model
The interpreter evaluates an abstract syntax tree (AST) in a top-down, recursive manner.
Statements are executed via the `execute()` method, and expressions are evaluated using
`eval_expr()`. Both methods operate over structured tuples representing nodes in the AST.

2. Environment
The interpreter maintains a dictionary `vars` representing the current
variable environment (scope). Function calls temporarily replace this
environment to simulate local scope and then restore it after the call
completes.

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


File: interpreter.py
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © 2025 Chris Rowles. All rights reserved.
Version: 0.1.0
License: MIT
"""

import os

from omglang.exceptions import (
    UndefinedVariableException,
    UnknownOpException,
    BreakLoop,
    ReturnControlFlow,
)
from omglang.operations import Op


class FrozenNamespace(dict):
    """Dictionary that disallows modification."""

    def __readonly(self, *_args, **_kwargs):  # type: ignore[no-untyped-def]
        raise TypeError("Imported modules are read-only")

    __setitem__ = __readonly  # type: ignore[assignment]
    __delitem__ = __readonly  # type: ignore[assignment]
    pop = __readonly  # type: ignore[assignment]
    popitem = __readonly  # type: ignore[assignment]
    clear = __readonly  # type: ignore[assignment]
    update = __readonly  # type: ignore[assignment]


class FunctionValue:
    """Runtime representation of a function value."""

    def __init__(self, params, body, env, global_env):
        self.params = params
        self.body = body
        self.env = env
        # Reference to the global namespace in which the function was defined.
        self.global_env = global_env


class Interpreter:
    """Tree-walk interpreter for OMGlang."""

    def __init__(self, file: str, loaded_modules: set[str] | None = None):
        """Initialize the interpreter."""
        self.vars = {}
        self.global_vars = self.vars
        self.file = file
        self.loaded_modules = loaded_modules if loaded_modules is not None else set()

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

    # ------------------------------------------------------------------
    # Module system
    # ------------------------------------------------------------------

    def import_module(self, path: str) -> FrozenNamespace:
        """
        Load another OMG script and return its exported namespace.

        Args:
            path (str): The relative or absolute path to the module.

        Returns:
            FrozenNamespace: A read-only namespace containing the module's exported variables.
        """
        base_dir = os.path.dirname(self.file) if self.file not in {"<stdin>", "<test>"} else os.getcwd()
        module_path = os.path.normpath(os.path.abspath(os.path.join(base_dir, path)))

        if module_path in self.loaded_modules:
            raise RuntimeError(f"Recursive import of '{module_path}'")
        if not os.path.exists(module_path):
            raise FileNotFoundError(f"Module '{path}' not found relative to '{self.file}'")

        self.loaded_modules.add(module_path)
        try:
            from omglang.lexer import tokenize
            from omglang.parser import Parser

            with open(module_path, "r", encoding="utf-8") as f:
                code = f.read()

            module_interpreter = Interpreter(module_path, self.loaded_modules)
            module_interpreter.vars["args"] = []
            module_interpreter.check_header(code)

            tokens, token_map = tokenize(code)
            parser = Parser(tokens, token_map, module_path)
            ast = parser.parse()

            exports: set[str] = set()
            for stmt in ast:
                if stmt[0] == "decl":
                    exports.add(stmt[1])
                elif stmt[0] == "func_def":
                    exports.add(stmt[1])

            module_interpreter.execute(ast)

            exported_bindings = {name: module_interpreter.vars.get(name) for name in exports}
            return FrozenNamespace(exported_bindings)
        except SyntaxError as e:
            raise SyntaxError(f"Error in module '{module_path}': {e}") from e
        finally:
            self.loaded_modules.remove(module_path)

    def _format_expr(self, node) -> str:
        """
        Convert AST back to a readable string for debugging.

        Args:
            node (tuple): An expression node, structured as a tuple.

        Returns:
            str: A string representation of the expression.
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
            case 'string':
                return repr(node[1])
            case 'list':
                return '[' + ', '.join(self._format_expr(e) for e in node[1]) + ']'
            case 'dict':
                return '{' + ', '.join(f"{k}: {self._format_expr(v)}" for k, v in node[1]) + '}'
            case 'index':
                return f"{self._format_expr(node[1])}[{self._format_expr(node[2])}]"
            case 'slice':
                start = self._format_expr(node[2])
                end = '' if node[3] is None else self._format_expr(node[3])
                return f"{self._format_expr(node[1])}[{start}:{end}]"
            case 'dot':
                return f"{self._format_expr(node[1])}.{node[2]}"
            case 'func_call':
                func_expr, args, _ = node[1], node[2], node[3]
                return (
                    f"{self._format_expr(func_expr)}"
                    f"({', '.join(self._format_expr(arg) for arg in args)})"
                )
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
            elif op == 'dict':
                _, pairs, _ = node
                return {k: self.eval_expr(v) for k, v in pairs}

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
                _, target_node, index_expr, _ = node
                target = self.eval_expr(target_node)
                index = self.eval_expr(index_expr)
                if isinstance(target, list):
                    if not 0 <= index < len(target):
                        raise RuntimeError(
                            f"List index out of bounds!\n"
                            f"{self._format_expr(node)}\n"
                            f"On line {line} in {self.file}"
                        )
                    return target[index]
                if isinstance(target, str):
                    if not 0 <= index < len(target):
                        raise RuntimeError(
                            f"String index out of bounds!\n"
                            f"{self._format_expr(node)}\n"
                            f"On line {line} in {self.file}"
                        )
                    return target[index]
                if isinstance(target, dict):
                    if index not in target:
                        raise KeyError(
                            f"Key '{index}' not found on line {line} in {self.file}"
                        )
                    return target[index]
                raise TypeError(
                    f"{self._format_expr(target_node)} is not indexable!\n"
                    f"{self._format_expr(node)}\n"
                    f"On line {line} in {self.file}"
                )

            # Index slicing
            elif op == 'slice':
                _, target_node, start_expr, end_expr, _ = node
                target = self.eval_expr(target_node)
                start = self.eval_expr(start_expr)
                end = self.eval_expr(end_expr) if end_expr is not None else None
                if isinstance(target, (list, str)):
                    return target[start:end]
                raise TypeError(
                    f"{self._format_expr(target_node)} is not sliceable!\n"
                    f"{self._format_expr(node)}\n"
                    f"On line {line} in {self.file}"
                )
            elif op == 'dot':
                _, target_node, attr_name, _ = node
                target = self.eval_expr(target_node)
                if not isinstance(target, dict):
                    raise TypeError(
                        f"{self._format_expr(target_node)} has no attribute '{attr_name}'\n"
                        f"On line {line} in {self.file}"
                    )
                if attr_name not in target:
                    raise KeyError(
                        f"Key '{attr_name}' not found on line {line} in {self.file}"
                    )
                return target[attr_name]

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
                Op.OR,
            ):
                lhs = self.eval_expr(node[1])
                if op == Op.AND:
                    if not bool(lhs):
                        return False
                    rhs = self.eval_expr(node[2])
                    return bool(rhs)
                elif op == Op.OR:
                    if bool(lhs):
                        return True
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
                _, func_node, args_nodes, line = node
                args = [self.eval_expr(arg) for arg in args_nodes]

                # Handle built-in functions before resolving variables
                if func_node[0] == 'ident':
                    func_name = func_node[1]
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

                    if func_name == 'read_file':
                        if len(args) != 1 or not isinstance(args[0], str):
                            raise TypeError(
                                f"read_file() expects a file path string!\n"
                                f"on line {line} in {self.file}"
                            )
                        with open(args[0], 'r', encoding='utf-8') as f:
                            return f.read()

                # User-defined functions
                func_value = self.eval_expr(func_node)
                if not isinstance(func_value, FunctionValue):
                    raise TypeError(
                        f"Attempted to call non-function '{self._format_expr(func_node)}'\n"
                        f"On line {line} in {self.file}"
                    )

                params, body, env = (
                    func_value.params,
                    func_value.body,
                    func_value.env,
                )
                func_globals = func_value.global_env

                if len(args) != len(params):
                    raise TypeError(
                        f"Function expects {len(params)} arguments "
                        f"{self._format_expr(node)}\n"
                        f"On line {line} in {self.file}"
                    )

                saved_vars = self.vars
                saved_globals = self.global_vars
                self.vars = env.copy()
                self.vars.update(dict(zip(params, args)))
                self.global_vars = func_globals

                try:
                    if body[0] == "block":
                        self.execute(body[1])
                    else:
                        self.execute([body])
                    result = None
                except ReturnControlFlow as ret:
                    result = ret.value
                finally:
                    self.vars = saved_vars
                    self.global_vars = saved_globals

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
            elif kind == 'attr_assign':
                _, obj_expr, attr_name, value_expr, _ = stmt
                target = self.eval_expr(obj_expr)
                if not isinstance(target, dict):
                    raise TypeError(
                        f"{self._format_expr(obj_expr)} has no attribute '{attr_name}'\n"
                        f"On line {line} in {self.file}"
                    )
                target[attr_name] = self.eval_expr(value_expr)
            elif kind == 'index_assign':
                _, obj_expr, key_expr, value_expr, _ = stmt
                target = self.eval_expr(obj_expr)
                key = self.eval_expr(key_expr)
                value = self.eval_expr(value_expr)
                if isinstance(target, dict):
                    target[key] = value
                elif isinstance(target, list):
                    if not isinstance(key, int):
                        raise TypeError(
                            f"List index must be int on line {line} in {self.file}"
                        )
                    if not 0 <= key < len(target):
                        raise RuntimeError(
                            f"List index out of bounds on line {line} in {self.file}"
                        )
                    target[key] = value
                else:
                    raise TypeError(
                        f"{self._format_expr(obj_expr)} is not indexable on line {line} in {self.file}"
                    )

            elif kind == 'import':
                _, path, alias, _ = stmt
                module_ns = self.import_module(path)
                self.vars[alias] = module_ns

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
                captured = {} if self.vars is self.global_vars else self.vars.copy()
                # Preserve the global namespace where the function was defined so that
                # recursive calls and references to module-level bindings resolve correctly.
                func_value = FunctionValue(params, body, captured, self.global_vars)
                self.vars[name] = func_value

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
