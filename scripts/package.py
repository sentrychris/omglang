"""
Build script for the OMG runtime.

This script automates the process of downloading UPX (if needed), cleaning previous build 
artifacts, and building the OMG runtime executable.

Usage:
    To build runtime executable:
    python build.py --build [--clean] [--upx VERSION] [--upx-clean]

    To clean previous builds without building new executables:
    python build.py --clean

    To insert docstring headers into source files (no build):
    python build.py --insert-docstrings

    # To generate the third party licenses file (no build):
    python build.py --third-party-licenses

    # To generate the project directory tree (no build):
    python build.py --project-tree

Arguments:
    --build TYPE                 Specify the build type: "gui" or "headless"
    --clean                      Delete previous build and dist directories before building
    --upx VERSION                Specify the UPX version to download and use (default: 5.0.1)
    --upx-clean                  Delete the UPX directory in package_resources after building
    --insert-docstrings          Insert docstrings into .py source files (no build)
    --third-party-licenses       Generate third-party licenses file (no build)
    --project-tree               Generate project directory tree (no build)

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

DEFAULT_UPX_VER="5.0.2"


def get_upx(package_resources: str, upx_pkg: str, upx_url: str, is_windows: bool) -> str:
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


def clean_dir(directory: str) -> None:
    """
    Deletes the specified directory and all its contents if it exists.

    Args:
        directory (str): The directory to be cleaned.
    """

    if os.path.exists(directory):
        print(f"Cleaning {directory} directory...")
        shutil.rmtree(directory)


def build_exe(spec_file: str, upx_dir: str, dist_dir: str, build_dir: str) -> None:
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


def main(
        is_building: bool,
        clean_build: bool,
        clean_upx: bool,
        upx_ver: str,
        insert_docstrings_only: bool = False,
        third_party_licenses_only: bool = False,
        generate_project_tree_only: bool = False
    ) -> None:
    """
    Main function that orchestrates the build process for OMG.

    Args:
        is_building (bool): True if building the project, False otherwise.
        clean_build (bool): Clean `build` and `dist` directories.
        clean_upx (bool): Delete the UPX directory after building
        upx_ver (str): The version of UPX to use to compress the executable.
        insert_docstrings_only (bool): Insert docstrings into source files instead.
        third_party_licenses_only (bool): Generate third-party licenses instead.
        generate_project_tree_only (bool): Generate project directory tree instead.
    """
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out_dir = os.path.join(root, "output")

    # Handle insert docstrings only (no build)
    if insert_docstrings_only:
        print("Inserting docstrings into source .py files...")
        insert_docstrings()
        return

    # Handle generating licenses only (no build)
    if third_party_licenses_only:
        print("Generating third-party licenses file...")
        generate_third_party_licenses()
        return

    # Handle clean previous builds only (no build)
    if clean_build and not is_building:
        print("Cleaning previous build directories...")
        clean_dir(out_dir)
        return

    # Handle generating project directory tree only (no build)
    if generate_project_tree_only:
        print("Generating project directory tree...")
        write_tree_to_file()
        return

    if not is_building:
        return

    # Right... Now we're building... Make sure the build type is valid
    package_resources = os.path.join(root, "package_resources")
    build_spec = os.path.join(package_resources, "omg.spec")

    if not os.path.exists(build_spec):
        raise FileNotFoundError(f".spec file not found: {build_spec}")

    dist_dir = os.path.join(out_dir, "dist")
    build_dir = os.path.join(out_dir, "build")

    # Handle clean previous builds before new build
    if clean_build:
        print("Cleaning previous build directories...")
        clean_dir(dist_dir)
        clean_dir(build_dir)

    # Handle fetching UPX
    if os.name == "nt":
        upx_pkg = f"upx-{upx_ver}-win64"
        upx_url = f"https://github.com/upx/upx/releases/download/v{upx_ver}/{upx_pkg}.zip"
    else:
        upx_pkg = f"upx-{upx_ver}-amd64_linux"
        upx_url = f"https://github.com/upx/upx/releases/download/v{upx_ver}/{upx_pkg}.tar.xz"

    upx_dir = get_upx(package_resources, upx_pkg, upx_url, os.name == "nt")

    build_exe(build_spec, upx_dir, dist_dir, build_dir)

    if clean_upx:
        clean_dir(upx_dir)


def cli():
    """
    Command-line interface for the OMG build script.
    """
    parser = argparse.ArgumentParser(description="OMG packaging & utilities.")

    parser.add_argument(
        "--build",
        action="store_true",
        help="Build type (gui or headless)"
    )

    parser.add_argument(
        "--clean",
        action="store_true",
        help="Clean build and dist directories before building"
    )

    parser.add_argument(
        "--upx",
        metavar="VERSION",
        type=str,
        default=DEFAULT_UPX_VER,
        help="Specify UPX version (default: 5.0.1)"
    )

    parser.add_argument(
        "--upx-clean",
        action="store_true",
        help="Clean copy of UPX before building"
    )

    parser.add_argument(
        "--insert-docstrings",
        action="store_true",
        help="Insert docstrings into source files instead of building"
    )

    parser.add_argument(
        "--third-party-licenses",
        action="store_true",
        help="Generate third-party licenses file instead of building"
    )

    parser.add_argument(
        "--project-tree",
        action="store_true",
        help="Generate project directory tree"
    )

    args = parser.parse_args()

    main(
        is_building=args.build,
        clean_build=args.clean,
        clean_upx=args.upx_clean,
        upx_ver=args.upx,
        insert_docstrings_only=args.insert_docstrings,
        third_party_licenses_only=args.third_party_licenses,
        generate_project_tree_only=args.project_tree
    )


if __name__ == "__main__":
    cli()
