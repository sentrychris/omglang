"""
OLI - OMG Language Interpreter

This is the main entry point for the OMG language interpreter.

Workflow:
1. The source script is read from the file specified on the command line.
2. The Interpreter verifies the script header to ensure validity.
3. The Lexer tokenizes the stripped source code into meaningful tokens.
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
    print("OLI - OMG Language Interpreter")
    print()
    print("Usage:")
    print("    oli <script.omg>")
    print()
    print("Arguments:")
    print("    <script.omg>")
    print("        The path to an OMG language source file to execute. The file must")
    print("        include the required header ';;;omg' on the first non-empty line.")
    print()
    print("Example:")
    print("    oli hello.omg")
    print()
    print("Or run with no arguments to enter interactive mode (REPL).")


def run_script(script_name: str):
    """
    Run an OMG script
    """
    with open(script_name, "r", encoding="utf-8") as f:
        code = f.read()

    try:
        interpreter = Interpreter(script_name)
        interpreter.check_header(code)

        tokens = tokenize(interpreter.strip_header(code))
        parser = Parser(tokens, script_name)
        ast = parser.parse()

        interpreter.execute(ast)
    except Exception as e:
        print(f"{type(e).__name__}: {e}")


def run_repl():
    """
    Run the interactive REPL
    """
    print("OMG Language Intrepeter - REPL")
    print("Type `exit` or `quit` to leave.")
    interpreter = Interpreter("<stdin>")
    buffer = []

    while True:
        try:
            prompt = ">>> " if not buffer else "... "
            line = input(prompt)
            if line.strip() in {"exit", "quit"}:
                break
            buffer.append(line)
            source = "\n".join(buffer)

            try:
                tokens = tokenize(source)
                parser = Parser(tokens, "<stdin>")
                ast = parser.parse()

                interpreter.execute(ast)
                buffer.clear()
            except SyntaxError as e:
                if "unexpected EOF" in str(e).lower():
                    continue  # likely incomplete input
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


if __name__ == "__main__":
    if len(sys.argv) == 2:
        run_script(os.path.join(LAUNCH_ENV, sys.argv[1]))
    elif len(sys.argv) == 1:
        run_repl()
    else:
        print_usage()
        sys.exit(1)
