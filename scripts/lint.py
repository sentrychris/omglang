"""
Lint script runner.
"""
import subprocess

def main():
    """
    Lint the OMG project using flake8 and pylint.
    """
    print("Running flake8...")
    subprocess.run([
        "flake8",
        "./omglang",
        "./omg.py"
    ], check=True)

    print("Running pylint...")
    subprocess.run([
        "pylint",
        "./omglang",
        "./omg.py"
    ], check=True)
