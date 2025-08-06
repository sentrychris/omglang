import os
from pathlib import Path

# Directories to skip entirely
SKIP_DIRS = {"__pycache__", "node_modules", ".git", ".venv", ".github", ".pytest_cache", "script"}

def tree(dir_path: Path, prefix: str = "", file_lines: list[str] = []) -> None:
    """Recursively build a tree structure of the directory."""
    try:
        contents = sorted(
            [p for p in dir_path.iterdir() if p.name not in SKIP_DIRS],
            key=lambda p: (p.is_file(), p.name.lower())
        )
    except PermissionError:
        return

    for index, path in enumerate(contents):
        connector = "└── " if index == len(contents) - 1 else "├── "
        display = f"{prefix}{connector}{path.name}"
        file_lines.append(display)

        if path.is_dir():
            extension = "    " if index == len(contents) - 1 else "│   "
            tree(path, prefix + extension, file_lines)

def write_tree_to_file(output_file: str = "project_structure.txt") -> None:
    base_dir = Path.cwd()
    lines = [f"{base_dir.name}/"]
    tree(base_dir, "", lines)
    Path(output_file).write_text("\n".join(lines), encoding="utf-8")
    print(f"[✓] Project structure written to {output_file}")

if __name__ == "__main__":
    write_tree_to_file()