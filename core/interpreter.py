"""
Interpreter.
"""
class Interpreter:
    """
    A simple expression interpreter supporting variables and arithmetic.
    """
    def __init__(self):
        """
        Initialize the interpreter with an empty variable environment.
        """
        self.vars = {}


    def eval_expr(self, node: int | str | tuple):
        """
        Evaluate an expression node.

        Parameters:
            node (int | str | tuple): The expression node.

        Returns:
            The result of evaluating the expression.

        Raises:
            NameError: If a variable is used before assignment.
            Exception: For unknown node types.
        """
        if isinstance(node, int):
            return node
        if isinstance(node, str):
            return node
        if isinstance(node, tuple):
            op = node[0]
            ret = None
            if op == 'var':
                varname = node[1]
                if varname in self.vars:
                    ret = self.vars[varname]
                else:
                    raise NameError(f"Undefined variable '{varname}'")
            elif op == 'add':
                ret = self.eval_expr(node[1]) + self.eval_expr(node[2])
            elif op == 'sub':
                ret = self.eval_expr(node[1]) - self.eval_expr(node[2])
            elif op == 'mul':
                ret = self.eval_expr(node[1]) * self.eval_expr(node[2])
            elif op == 'div':
                ret = self.eval_expr(node[1]) / self.eval_expr(node[2])
            if ret is None:
                raise ValueError(f'Unknown op {op}')
            return ret
        raise ValueError(f'Unknown node {node}')

    def run(self, statements: list):
        """
        Execute a list of statements.

        Parameters:
            statements (list): A list of ('assign' | 'cout', ...) tuples.

        Raises:
            Exception: For unknown statement types.
        """
        for stmt in statements:
            kind = stmt[0]
            if kind == 'assign':
                _, var_name, expr_node = stmt
                value = self.eval_expr(expr_node)
                self.vars[var_name] = value
            elif kind == 'cout':
                _, expr_node = stmt
                value = self.eval_expr(expr_node)
                print(value)
            else:
                raise TypeError(f'Unknown statement type: {kind}')
