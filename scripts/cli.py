"""
Build utilities for OMG.

The script handles platform differences for UPX download URLs and extraction.
"""

import os
import sys
import shutil
import struct
import subprocess
import urllib.request
import zipfile
import tarfile
import argparse

from omglang.compiler import main as compile_interp, disassemble

from scripts.generate_docstring_headers import insert_docstrings
from scripts.generate_third_party_licenses_file import generate_third_party_licenses
from scripts.generate_project_tree import write_tree_to_file
from scripts.verify_omgb_file_bytes import verify_interpreter


# Project root
BASE_DIR=os.path.dirname(os.path.dirname(__file__))

# OMG python runtime + interpreter (Python + Python)
OMG_PY_ENTRYPOINT=os.path.join(BASE_DIR, 'omg.py')
OMG_PY_INTERPRETER_SRC=os.path.join(BASE_DIR, 'omglang')
DEFAULT_UPX_VER="5.0.2" # used by Pyinstaller for compression (check CFG)

# OMG native runtime + interpreter (Rust + OMG)
OMG_INTERPRETER_SRC=os.path.join(BASE_DIR, 'bootstrap', 'interpreter.omg')
OMG_INTERPRETER_BIN=os.path.join(BASE_DIR, 'runtime', 'interpreter.omgb')
RUNTIME_MANIFEST_PATH=os.path.join(BASE_DIR, 'runtime', 'Cargo.toml')


def _get_upx(package_resources: str, upx_pkg: str, upx_url: str, is_windows: bool) -> str:
    """
    Checks for the presence of UPX, downloads and extracts it if not present.

    Args:
        package_resources (str): The build resources directory where UPX will be located.
        upx_pkg (str): The archive name of the UPX version to download.
        upx_url (str): The URL from which to download UPX.
        is_windows (bool): True if the script is running on Windows, False otherwise.

    Returns:
        upx_dir (str): The directory where UPX is located.
    """

    upx_dir = os.path.join(package_resources, upx_pkg)

    if not os.path.exists(upx_dir):
        print("⏳ Downloading UPX...")
        upx_path = os.path.join(package_resources, f"{upx_pkg}.{'zip' if is_windows else 'tar.xz'}")
        urllib.request.urlretrieve(upx_url, upx_path)

        if is_windows:
            with zipfile.ZipFile(upx_path, "r") as zip_ref:
                zip_ref.extractall(package_resources)
        else:
            with tarfile.open(upx_path, "r:xz") as tar_ref:
                tar_ref.extractall(package_resources)

        os.remove(upx_path)
    else:
        print("ℹ️  UPX is available")

    return upx_dir


def _clean_dir(directory: str) -> None:
    """
    Deletes the specified directory and all its contents if it exists.

    Args:
        directory (str): The directory to be cleaned.
    """

    if os.path.exists(directory):
        print(f"Cleaning {directory} directory...")
        shutil.rmtree(directory)


def _build_exe(spec_file: str, upx_dir: str, dist_dir: str, build_dir: str) -> None:
    """
    Builds the OMG runtime using PyInstaller.

    Args:
        spec_file (str): The path to the PyInstaller spec file.
        upx_dir (str): The directory where UPX is located.
        dist_dir (str): Output directory for final executable.
        build_dir (str): Directory for PyInstaller's build artifacts.
    """

    print("⏳ Building OMG runtime executable...")
    subprocess.run([
        "pyinstaller",
        spec_file,
        "--upx-dir", upx_dir,
        "--distpath", dist_dir,
        "--workpath", build_dir
    ], check=True)


def _compile_native_interpreter(src: str, out_bin: str) -> None:
    print(f"⏳ Compiling native {src} for runtime...")
    compile_interp([src, out_bin])
    print(f"✅ Compiled to {out_bin}")
    print("\n❔ Verify the binary with `omg-cli verify <path>`")
    print("    <path> will default to `./runtime/interpreter.omgb` if not set.")


def _disassemble_native_interpreter(
    bin_path: str, start: int | None, end: int | None
) -> None:
    print(f"⏳ Disassembling {bin_path}..")
    with open(bin_path, "rb") as b:
        data = b.read()
    source = disassemble(data)
    if start is None and end is None:
        print("----------------------------------------------")
        print("📑             File Header                  📑")
        print("----------------------------------------------")
        header = data[:4]
        version = struct.unpack_from("<I", data, 4)[0]
        func_count = struct.unpack_from("<I", data, 8)[0]
        idx = 12
        print(f"magic {header!r}")
        print(f"version {version}")
        print("----------------------------------------------")
        print("🧬             Functions Table              🧬")
        print("----------------------------------------------")
        for _ in range(func_count):
            name_len = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            name = data[idx:idx + name_len].decode("utf-8")
            idx += name_len
            param_count = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            params = []
            for _ in range(param_count):
                p_len = struct.unpack_from("<I", data, idx)[0]
                idx += 4
                param = data[idx:idx + p_len].decode("utf-8")
                idx += p_len
                params.append(param)
            addr = struct.unpack_from("<I", data, idx)[0]
            idx += 4
            print(f"FUNC {name} {param_count} {' '.join(params)} {addr}")
    else:
        source = disassemble(data)
        s = 0 if start is None else start
        e = len(source) if end is None else end
        print(source[s:e])


def main():
    """
    Command-line interface for the OMG build script.
    """
    parser = argparse.ArgumentParser(
        description="OMG packaging & utilities.",
        allow_abbrev=False,
    )
    sub = parser.add_subparsers(dest="command", required=True)

    # docstring-headers
    sub.add_parser("docstring-headers", help="Insert docstring headers into .py source files")

    # third-party-licenses
    sub.add_parser("third-party-lics", help="Generate project third-party licenses file")

    # project-tree
    sub.add_parser("project-tree", help="Generate project directory tree representation")

    # lint-python
    sub.add_parser("lint-python", help="Lint .py source files with flake8 and pylint")

    # runtime-test
    sub.add_parser("runtime-test", help="Run tests for the Rust runtime")

    # runtime-run
    r_run = sub.add_parser("runtime-run", help="Execute the Rust runtime")
    r_run.add_argument(
        "src",
        nargs="?",
        default=None,
        help="Path to .omg source script"
    )

    # runtime-build
    r_build = sub.add_parser(
        "runtime-build", help="Build the OMG native runtime (with embedded interpreter.omgb).",
        description=(
            "This command builds the Rust-based runtime that OMG executes in. Runtime contains "
            "The VM, opcode instruction set, handlers and the compiled .omgb interpreter. "
            "The .omgb interpreter is compiled during the cargo build process and embedded."
        )
    )
    r_build.add_argument(
        "--dump-omgb",
        action="store_true",
        help=f"Dump the compiled interpreter.omgb binary to {OMG_INTERPRETER_BIN}"
    )
    r_build.add_argument(
        "--with-symbols",
        action="store_true",
        help="Build the runtime with symbols and info for debugging"
    )

    # compile-omgb
    p_compile = sub.add_parser(
        "compile-omgb", help="Compile an .omgb binary (interpreter by default)"
    )
    p_compile.add_argument(
        "src",
        nargs="?",
        default=OMG_INTERPRETER_SRC,
        help=f"Path to .omg source (default: {OMG_INTERPRETER_SRC})"
    )
    p_compile.add_argument(
        "-o", "--out",
        dest="out_bin",
        default=OMG_INTERPRETER_BIN,
        help=f"Output binary path (default: {OMG_INTERPRETER_BIN})"
    )

    # verify-omgb
    p_verify = sub.add_parser(
        "verify-omgb", help="Verify .omgb binary for the runtime (interpreter by default)"
    )
    p_verify.add_argument(
        "bin",
        nargs="?",
        default=OMG_INTERPRETER_BIN,
        help=f"Path to .omgb binary (default: {OMG_INTERPRETER_BIN})"
    )

    # disassemble-omgb
    p_dis = sub.add_parser(
        "disassemble-omgb", help="Disassemble a compiled .omgb binary (interpreter by default)"
    )
    p_dis.add_argument(
        "bin",
        nargs="?",
        default=OMG_INTERPRETER_BIN,
        help=f"Path to .omgb binary (default: {OMG_INTERPRETER_BIN})",
    )
    p_dis.add_argument(
        "--start",
        type=int,
        default=None,
        help="Start index into disassembly output",
    )
    p_dis.add_argument(
        "--end",
        type=int,
        default=None,
        help="End index into disassembly output",
    )

    # Legacy Python runtime embedded build
    p_build = sub.add_parser(
        "legacy-build", help="Legacy Python-based OMG interpreter",
        description=(
            "This command builds the original OMG interpreter which was implemented in "
            "Python (using PyInstaller). The resulting build embeds the entire Python "
            "runtime and is therefore quite a bit larger than the native runtime!"
        )
    )
    p_build.add_argument(
        "--clean",
        action="store_true",
        help="Clean build and dist directories before building"
    )
    p_build.add_argument(
        "--upx",
        metavar="VERSION",
        type=str,
        default=DEFAULT_UPX_VER,
        help=f"Specify UPX version (default: {DEFAULT_UPX_VER})"
    )
    p_build.add_argument(
        "--upx-clean",
        action="store_true",
        help="Delete the downloaded UPX directory after building"
    )

    args = parser.parse_args()

    # Common paths
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out_dir = os.path.join(root, "output")
    dist_dir = os.path.join(out_dir, "dist")
    build_dir = os.path.join(out_dir, "build")
    package_resources = os.path.join(root, "package_resources")
    build_spec = os.path.join(package_resources, "omg.spec")

    if args.command == "docstring-headers":
        print("⏳ Inserting docstrings into source .py files...")
        insert_docstrings()
        return

    if args.command == "third-party-lics":
        print("⏳ Generating third-party licenses file...")
        generate_third_party_licenses()
        return

    if args.command == "project-tree":
        print("⏳ Generating project directory tree...")
        write_tree_to_file()
        return

    if args.command == "lint-python":
        print("⏳ Running flake8...")
        subprocess.run(
            ["flake8", OMG_PY_INTERPRETER_SRC, OMG_PY_ENTRYPOINT],
            check=True
        )

        print("⏳ Running pylint...")
        subprocess.run(
            ["pylint", OMG_PY_INTERPRETER_SRC, OMG_PY_ENTRYPOINT],
            check=True
        )

    if args.command == "compile-omgb":
        _compile_native_interpreter(args.src, args.out_bin)
        return

    if args.command == "disassemble-omgb":
        _disassemble_native_interpreter(args.bin, args.start, args.end)
        return

    if args.command == "verify-omgb":
        print(f"⏳ Verifying {args.bin}")
        try:
            verify_interpreter(args.bin)
        except ValueError as e:
            print(f"ERROR! Failed to verify binary: {e}")
            sys.exit(1)
        return

    if args.command == "runtime-test":
        print("⏳ Running tests")
        subprocess.run(
            ['cargo', 'test', '--manifest-path', RUNTIME_MANIFEST_PATH],
            check=True
        )

    if args.command == "runtime-run":
        print("⏳ Executing runtime")
        p_args = ['cargo', 'run', '--manifest-path', RUNTIME_MANIFEST_PATH]
        if args.src:
            p_args.append('--')
            p_args.append(args.src)
        subprocess.run(p_args, check=True)

    if args.command == "runtime-build":
        env = os.environ.copy()
        if args.dump_omgb:
            env["DUMP_OMGB"] = "1"
        cmd = ["cargo", "build", "--release", "--manifest-path", RUNTIME_MANIFEST_PATH]
        if args.with_symbols:
            extra = "-C debuginfo=2 -C panic=abort"
            env["RUSTFLAGS"] = (env.get("RUSTFLAGS", "") + " " + extra).strip()
        subprocess.run(cmd, check=True, env=env)

    if args.command == "legacy-build":
        if not os.path.exists(build_spec):
            raise FileNotFoundError(f".spec file not found: {build_spec}")
        if args.clean:
            print("⏳ Cleaning previous build directories...")
            _clean_dir(dist_dir)
            _clean_dir(build_dir)
        # Fetch UPX (scoped to build only)
        if os.name == "nt":
            upx_pkg = f"upx-{args.upx}-win64"
            upx_url = f"https://github.com/upx/upx/releases/download/v{args.upx}/{upx_pkg}.zip"
        else:
            upx_pkg = f"upx-{args.upx}-amd64_linux"
            upx_url = f"https://github.com/upx/upx/releases/download/v{args.upx}/{upx_pkg}.tar.xz"
        upx_dir = _get_upx(package_resources, upx_pkg, upx_url, os.name == "nt")
        _build_exe(build_spec, upx_dir, dist_dir, build_dir)
        if args.upx_clean:
            _clean_dir(upx_dir)

if __name__ == "__main__":
    main()
