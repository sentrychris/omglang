"""
Prepend docstrings headers to all .py files in a directory tree.
"""
import datetime
import os
import re

current_year = datetime.datetime.now().year

def should_skip(path: str) -> bool:
    """
    Determine whether a file should be skipped based on its path.

    Args:
        path (str): The full path to the file.

    Returns:
        bool: True if the file should be skipped, False otherwise.
    """
    return "__pycache__" in path or not path.endswith(".py")


def prepend_header_to_file(filepath: str) -> None:
    """
    Add or update author/copyright/version/license block inside the module docstring.

    Args:
        filepath (str): The path to the file to modify.
    """
    filename = os.path.basename(filepath)
    new_footer = f"""File: {filename}
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © {current_year} Chris Rowles. All rights reserved.
Version: 0.1.1
License: MIT"""

    with open(filepath, "r", encoding="utf-8") as file:
        contents = file.read()

    # Match the first triple-quoted docstring at the top
    docstring_match = re.match(r'("""|\'\'\')([\s\S]*?)(\1)', contents)

    if docstring_match:
        quote = docstring_match.group(1)
        body = docstring_match.group(2)
        end_quote = docstring_match.group(3)

        # Regex to match previous footer — tolerates whitespace and line-endings
        footer_pattern = re.compile(
            r"File: .+?\nAuthor: .+?\nCopyright: .+?\nVersion: .+?\nLicense: .+?$",
            re.MULTILINE
        )

        if footer_pattern.search(body):
            updated_body = footer_pattern.sub(new_footer, body.strip())
            action = "📝 Footer block updated in"
        else:
            updated_body = body.strip() + "\n\n" + new_footer
            action = "📝 Footer block appended to"

        new_docstring = f"{quote}{updated_body}\n{quote}"
        new_contents = new_docstring + contents[docstring_match.end():]
    else:
        new_docstring = f'"""{new_footer}"""\n\n'
        new_contents = new_docstring + contents
        action = "📝 Docstring created with footer block"

    with open(filepath, "w", encoding="utf-8") as file:
        file.write(new_contents)

    print(f"{action}: {filepath}")



def process_directory(root: str) -> None:
    """
    Recursively process all Python files in a directory tree.

    Args:
        root (str): The root directory to start from.
    """
    for dirpath, dirnames, filenames in os.walk(root):
        # Skip directories
        dirnames[:] = [d for d in dirnames if d not in ["__pycache__", "tests"]]

        for filename in filenames:
            full_path = os.path.join(dirpath, filename)
            if should_skip(full_path):
                continue
            prepend_header_to_file(full_path)


def insert_docstrings():
    """
    Insert docstrings into source files.
    """
    project_root = os.path.join(os.getcwd(), "omglang")
    process_directory(project_root)


if __name__ == "__main__":
    insert_docstrings()