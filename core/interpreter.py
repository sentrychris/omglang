class Interpreter:
    def __init__(self):
        self.vars = {}

    def eval_expr(self, node):
        if isinstance(node, int):
            return node
        if isinstance(node, str):
            return node
        if isinstance(node, tuple):
            op = node[0]
            if op == 'var':
                varname = node[1]
                if varname in self.vars:
                    return self.vars[varname]
                else:
                    raise NameError(f"Undefined variable '{varname}'")
            elif op == 'add':
                return self.eval_expr(node[1]) + self.eval_expr(node[2])
            elif op == 'sub':
                return self.eval_expr(node[1]) - self.eval_expr(node[2])
            elif op == 'mul':
                return self.eval_expr(node[1]) * self.eval_expr(node[2])
            elif op == 'div':
                return self.eval_expr(node[1]) / self.eval_expr(node[2])
        raise Exception(f'Unknown node {node}')

    def run(self, statements):
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
                raise Exception(f'Unknown statement type: {kind}')

