"""
OMG Language Interpreter

This is the main entry point for the OMG language interpreter.

Workflow:
1. The source script is read from the file specified on the command line.
2. The Interpreter verifies the script header to ensure validity.
3. The Lexer tokenizes the source code into meaningful tokens.
4. The Parser processes tokens into an AST following the language grammar.
5. The Interpreter walks the AST, evaluating expressions and executing statements.
"""
import os
import sys

from core.lexer import tokenize
from core.parser import Parser
from core.interpreter import Interpreter

if getattr(sys, "frozen", False):
    # Running in a bundled executable
    LAUNCH_ENV = os.getcwd()
else:
    # Running through a Python interpreter
    LAUNCH_ENV = os.path.abspath(os.path.join(os.path.dirname(__file__)))


def print_usage():
    """
    Print usage.
    """
    print()
    print("OMG Language Interpreter")
    print()
    print("Usage:")
    print("    omg <script.omg>")
    print()
    print("Arguments:")
    print("    <script.omg>")
    print("        Path to an OMG language source file to execute. The file must")
    print("        include the required header ';;;omg' on the first non-empty line.")
    print()
    print("Example:")
    print("    omg hello.omg")
    print()
    print("Or run with no arguments to enter interactive mode (REPL).")
    print()
    print("Options:")
    print("    -h, --help")
    print("        Show this help message and exit.")


def debug_print_tokens_ast(tokens, ast):
    """
    Print tokenized source and AST
    """
    print ("\nTokens:\n")
    print(tokens)
    print ("\nAST:\n")
    print(ast)
    print (" ")

def run_script(script_name: str):
    """
    Run an OMG script
    """
    with open(script_name, "r", encoding="utf-8") as f:
        code = f.read()

    try:
        interpreter = Interpreter(script_name)
        interpreter.check_header(code)

        tokens, token_map_literals = tokenize(code)
        parser = Parser(tokens, token_map_literals, script_name)
        ast = parser.parse()

        if os.environ.get('OMGDEBUG'):
            debug_print_tokens_ast(tokens, ast)

        interpreter.execute(ast)
    except Exception as e:
        print(f"{type(e).__name__}: {e}")


def run_repl():
    """
    Run the interactive REPL
    """
    print("OMG Language Interpreter - REPL")
    print("Type `exit` or `quit` to leave.")
    interpreter = Interpreter("<stdin>")
    buffer: list[str] = []
    while True:
        try:
            prompt = ">>> " if not buffer else "... "
            line = input(prompt)
            if line.strip() in {"exit", "quit"}:
                break
            buffer.append(line)
            source = "\n".join(buffer)
            try:
                tokens, token_map_literals = tokenize(source)
                parser = Parser(tokens, token_map_literals, "<stdin>")
                ast = parser.parse()
                interpreter.execute(ast)
                buffer.clear()
            except SyntaxError as e:
                # If the parser complains about reaching EOF, assume the input is incomplete
                if "EOF" in str(e):
                    continue
                raise
            except Exception as e:
                print(f"{type(e).__name__}: {e}")
                buffer.clear()
        except KeyboardInterrupt:
            print("\nInterrupted.")
            break
        except EOFError:
            print()
            break


def main(argv: list[str]) -> int:
    """
    Entry point for the CLI.

    Behaviour:
    - No arguments: enter the REPL.
    - One argument equal to ``-h`` or ``--help``: print usage and exit.
    - One argument that is not an option: treat it as the path to a script and run it.
    - Any other pattern: print usage and return a non‑zero exit code.
    """
    args = argv[1:]
    if not args:
        run_repl()
        return 0
    if len(args) == 1 and args[0] in ('-h', '--help'):
        print_usage()
        return 0
    if len(args) == 1:
        run_script(args[0])
        return 0
    print_usage()
    return 1


if __name__ == "__main__":
    main(sys.argv)
