"""
Build utilities for OMG.

The script handles platform differences for UPX download URLs and extraction.
"""

import os
import shutil
import subprocess
import urllib.request
import zipfile
import tarfile
import argparse

from scripts.generate_docstring_headers import insert_docstrings
from scripts.generate_third_party_licenses_file import generate_third_party_licenses
from scripts.generate_project_tree import write_tree_to_file
from scripts.verify_binary import verify_interpreter

from omglang.compiler import main as compile_interp, disassemble

BASE_DIR=os.path.dirname(os.path.dirname(__file__))
DEFAULT_UPX_VER="5.0.2"
OMG_INTERPRETER_SRC=os.path.join(BASE_DIR, 'bootstrap', 'interpreter.omg')
OMG_INTERPRETER_BIN=os.path.join(BASE_DIR, 'runtime', 'interpreter.omgb')


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
        print("Downloading UPX...")
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
        print("UPX is available")

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

    print("Building OMG runtime executable...")
    subprocess.run([
        "pyinstaller",
        spec_file,
        "--upx-dir", upx_dir,
        "--distpath", dist_dir,
        "--workpath", build_dir
    ], check=True)


def _compile_native_interpreter(src: str, out_bin: str) -> None:
    print(f"Compiling native {src} for runtime...")
    compile_interp([src, out_bin])
    print(f"Compiled to {out_bin}")


def _disassemble_native_interpreter(bin_path: str) -> None:
    print("Disassembling the compiled native interpreter...")
    with open(bin_path, "rb") as b:
        data = b.read()
    source = disassemble(data)
    # Avoid dumping the whole thing
    print("...\n" + source[1500:1600] + "\n...")


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
    sub.add_parser("docstring-headers", help="Insert docstring headers into .py sources")

    # third-party-licenses
    sub.add_parser("third-party-lics", help="Generate third-party licenses file")

    # project-tree
    sub.add_parser("project-tree", help="Generate project directory tree representation")

    # compile native interpreter
    p_compile = sub.add_parser("compile", help="Compile the OMG interpreter for the runtime VM")
    p_compile.add_argument("src", nargs="?", default=OMG_INTERPRETER_SRC, help=f"Path to interpreter source (default: {OMG_INTERPRETER_SRC})")
    p_compile.add_argument("-o", "--out", dest="out_bin", default=OMG_INTERPRETER_BIN, help=f"Output binary path (default: {OMG_INTERPRETER_BIN})")

    # verify compiled interpreter
    p_verify = sub.add_parser("verify", help="Verify the compiled interpreter binary for the runtime VM")
    p_verify.add_argument("bin", nargs="?", default=OMG_INTERPRETER_BIN, help=f"Path to interpreter binary (default: {OMG_INTERPRETER_BIN})")

    # disassemble native interpreter
    p_dis = sub.add_parser("disassemble", help="Disassemble a compiled OMG interpreter binary")
    p_dis.add_argument("bin", nargs="?", default=OMG_INTERPRETER_BIN, help=f"Path to interpreter binary (default: {OMG_INTERPRETER_BIN})")

    # Legacy Python runtime embedded build
    p_build = sub.add_parser("legacy-build", help="Legacy Python-based OMG interpreter (embeds the Python runtime)")

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
        print("Inserting docstrings into source .py files...")
        insert_docstrings()
        return

    if args.command == "third-party-lics":
        print("Generating third-party licenses file...")
        generate_third_party_licenses()
        return

    if args.command == "project-tree":
        print("Generating project directory tree...")
        write_tree_to_file()
        return

    if args.command == "compile":
        _compile_native_interpreter(args.src, args.out_bin)
        return

    if args.command == "disassemble":
        _disassemble_native_interpreter(args.bin)
        return

    if args.command == "verify":
        print(f"Verifying {args.bin}")
        verify_interpreter(args.bin)
        return

    # build
    if args.command == "build":
        if not os.path.exists(build_spec):
            raise FileNotFoundError(f".spec file not found: {build_spec}")
        if args.clean:
            print("Cleaning previous build directories...")
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
