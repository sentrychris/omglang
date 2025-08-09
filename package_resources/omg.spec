"""
# omg.spec
"""
import os
import sys

cwd = os.getcwd()
omg_runtime = os.path.join(cwd, 'omg.py')

package_resources = os.path.join(cwd, 'package_resources')

version_file = os.path.join(
    package_resources,
    'windows',
    'version.rc'
) if sys.platform == 'win32' else None

icon_file = os.path.join(
    package_resources,
    'assets',
    'icon.ico'
)

datas = [
    (omg_runtime, 'omg'),
    (icon_file, 'icon.ico')
]

if sys.platform == 'win32':
    datas += [(version_file, 'version.rc')]

BLOCK_CIPHER = None

a = Analysis(
    [omg_runtime],
    pathex=[cwd],
    binaries=[],
    datas=datas,
    hiddenimports=[],
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
    cipher=BLOCK_CIPHER,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=BLOCK_CIPHER)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='omg',
    icon=icon_file if sys.platform == 'win32' else None,
    version=version_file if sys.platform == 'win32' else None,
    console=True,
    debug=False,
    upx=True,
)
