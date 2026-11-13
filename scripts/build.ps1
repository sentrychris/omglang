# Build the OMG native toolchain on Windows: bin\omg.exe plus
# the two runtime files it pastes into the code it generates
# (omg_rt.h for C, omg_rt.js for JavaScript).
#
# This is scripts/build.sh for PowerShell; the two do the same thing.
#
#     powershell -ExecutionPolicy Bypass -File scripts\build.ps1
#
# Needs GCC or Clang from MinGW-w64 on PATH (WinLibs, MSYS2, w64devkit,
# llvm-mingw). Microsoft's cl.exe is not supported: the C runtime uses
# GNU C extensions.
#
# The whole build is two commands:
#
#     omg --emit-c src\omg.omg omg.c                  # OMG -> C
#     gcc -O3 omg.c -o bin\omg.exe -static -lm -lws2_32
#
# and the only question is which `omg` runs the first one:
#   1. Self-rebuild: bin\omg.exe compiles its own source.
#   2. Bootstrap: there is no usable omg.exe yet, so the Rust runtime
#      at rust\target\release\omg.exe runs omg.omg from source.
#
# bin\omg.exe is checked in (it is built on Linux by
# build.sh with a cross compiler, together with the Linux binary), so
# a fresh clone takes path 1. A binary built here is for this machine;
# don't commit it back.

$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$SrcDir = 'src'
$BinDir = 'bin'
$OmgExe = "$BinDir/omg.exe"
$RustOmg = 'rust/target/release/omg.exe'
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

$CC = $null
foreach ($name in @('gcc', 'clang', 'cc')) {
    if (Get-Command $name -ErrorAction SilentlyContinue) {
        $CC = $name
        break
    }
}
if (-not $CC) {
    Write-Host 'No C compiler found (looked for: gcc, clang, cc).'
    Write-Host 'Install GCC or Clang from MinGW-w64 and put its bin folder on PATH,'
    Write-Host 'for example:  winget install BrechtSanders.WinLibs.POSIX.UCRT'
    exit 1
}

# An omg.exe from before --emit-c existed can run and still be unable
# to build the toolchain, so ask it.
function Test-CanEmitC([string]$Exe) {
    if (-not (Test-Path $Exe)) { return $false }
    try {
        $help = & $Exe --help
        if ($LASTEXITCODE -ne 0) { return $false }
        return (($help -join "`n") -match '--emit-c')
    } catch {
        return $false
    }
}

if (Test-CanEmitC $OmgExe) {
    Write-Host "[1/3] Self-rebuild: $OmgExe compiles its own source"
    $Omg = @($OmgExe)
} elseif (Test-Path $RustOmg) {
    Write-Host '[1/3] Bootstrapping from the Rust runtime'
    # The Rust runtime runs the driver from source. `--rust` picks its
    # Rust frontend to load omg.omg, which takes milliseconds; the
    # compile of the toolchain itself is still done by compiler.omg.
    $Omg = @($RustOmg, '--rust', "$SrcDir/omg.omg")
} else {
    Write-Host "Need either a working $OmgExe or the Rust runtime at $RustOmg."
    Write-Host 'First-time build: cargo build --release --manifest-path rust/Cargo.toml'
    exit 1
}

$Work = Join-Path ([System.IO.Path]::GetTempPath()) ('omg-build-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Work | Out-Null

try {
    # Install the runtime files FIRST. The driver reads omg_rt.h (and
    # omg_rt.js) from the directory it lives in and pastes it into what
    # it generates, so a change to either must be in place before the
    # rebuilt binary is generated.
    Write-Host '[2/3] Installing runtime files'
    Copy-Item -Force "$SrcDir/runtime/omg_rt.h" "$BinDir/omg_rt.h"
    Copy-Item -Force "$SrcDir/runtime/omg_rt.js" "$BinDir/omg_rt.js"

    Write-Host "[3/3] Building omg.exe with $CC"
    $OmgArgs = @()
    if ($Omg.Count -gt 1) { $OmgArgs = $Omg[1..($Omg.Count - 1)] }
    & $Omg[0] @OmgArgs --emit-c "$SrcDir/omg.omg" "$Work/omg.c"
    if ($LASTEXITCODE -ne 0) { throw "omg --emit-c failed (exit code $LASTEXITCODE)" }

    # Build under a temporary name and move it into place. By now the
    # omg.exe that wrote omg.c has exited, so it can be replaced.
    & $CC -O3 -w "$Work/omg.c" -o "$Work/omg-new.exe" -static -lm -lws2_32
    if ($LASTEXITCODE -ne 0) { throw "$CC failed (exit code $LASTEXITCODE)" }
    Move-Item -Force "$Work/omg-new.exe" $OmgExe
} finally {
    Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}

Write-Host ''
Write-Host "Native toolchain in ${BinDir}:"
Get-ChildItem $BinDir | Format-Table Name, Length, LastWriteTime -AutoSize | Out-Host
Write-Host 'Run a program with:    bin\omg.exe foo.omg'
Write-Host 'Build an executable:   bin\omg.exe --build foo.omg'
