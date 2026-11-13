# Build the OMG native toolchain on Windows: bin\omg.exe plus the
# runtime files it adds to what it generates (x64\*.omg for machine
# code, omg_rt.h for C, omg_rt.js for JavaScript).
#
# This is scripts/build.sh for PowerShell; the two do the same thing.
#
#     powershell -ExecutionPolicy Bypass -File scripts\build.ps1
#
# Nothing else needs to be installed: omg.exe writes executables
# itself, its own included.
#
#     omg --build src\omg.omg bin\omg.exe
#
# The only question is which `omg` does the building:
#   1. Self-rebuild: bin\omg.exe builds its own source.
#   2. Bootstrap: there is no usable omg.exe, and someone has built
#      the frozen Rust runtime (see rust\README.md), so
#      rust\target\release\omg.exe runs omg.omg from source.
#
# bin\omg.exe is checked in (scripts/build.sh writes it on Linux,
# together with the Linux binary), so a fresh clone takes path 1.
#
# `scripts\build.ps1 -Target c` builds through C instead, which needs
# GCC or Clang from MinGW-w64 on PATH (WinLibs, MSYS2, w64devkit,
# llvm-mingw; Microsoft's cl.exe is not supported, because the C
# runtime uses GNU C extensions):
#
#     omg --emit-c src\omg.omg omg.c                  # OMG -> C
#     gcc -O3 omg.c -o bin\omg.exe -static -lm -lws2_32

param(
    [ValidateSet('windows-x64', 'c')]
    [string]$Target = 'windows-x64'
)

$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$SrcDir = 'src'
$BinDir = 'bin'
$OmgExe = "$BinDir/omg.exe"
$RustOmg = 'rust/target/release/omg.exe'
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

$CC = $null
if ($Target -eq 'c') {
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
}

# An omg.exe from before the step it is needed for existed can run and
# still be unable to build the toolchain, so ask it.
function Test-CanDo([string]$Exe, [string]$Flag) {
    if (-not (Test-Path $Exe)) { return $false }
    try {
        $help = & $Exe --help
        if ($LASTEXITCODE -ne 0) { return $false }
        return (($help -join "`n").Contains($Flag))
    } catch {
        return $false
    }
}
$Needs = '--target'
if ($Target -eq 'c') { $Needs = '--emit-c' }

if (Test-CanDo $OmgExe $Needs) {
    Write-Host "[1/3] Self-rebuild: $OmgExe builds its own source"
    $Omg = @($OmgExe)
} elseif (Test-Path $RustOmg) {
    Write-Host '[1/3] Bootstrapping from the Rust runtime'
    # The Rust runtime runs the driver from source. `--rust` picks its
    # Rust frontend to load omg.omg, which takes milliseconds; the
    # compile of the toolchain itself is still done by compiler.omg.
    $Omg = @($RustOmg, '--rust', "$SrcDir/omg.omg")
} else {
    Write-Host "There is no working $OmgExe. It is checked in: restore it with"
    Write-Host '    git checkout HEAD -- bin'
    Write-Host '(or build the frozen Rust runtime: see rust\README.md).'
    exit 1
}

$Work = Join-Path ([System.IO.Path]::GetTempPath()) ('omg-build-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Work | Out-Null

try {
    # Install the runtime files FIRST. The driver reads them from the
    # directory it lives in and builds them into what it generates, so
    # a change to any of them must be in place before the rebuilt
    # binary is generated.
    Write-Host '[2/3] Installing runtime files'
    Copy-Item -Force "$SrcDir/runtime/omg_rt.h" "$BinDir/omg_rt.h"
    Copy-Item -Force "$SrcDir/runtime/omg_rt.js" "$BinDir/omg_rt.js"
    if (Test-Path "$BinDir/x64") { Remove-Item -Recurse -Force "$BinDir/x64" }
    New-Item -ItemType Directory -Force -Path "$BinDir/x64" | Out-Null
    Copy-Item -Force "$SrcDir/runtime/x64/*.omg" "$BinDir/x64/"

    $OmgArgs = @()
    if ($Omg.Count -gt 1) { $OmgArgs = $Omg[1..($Omg.Count - 1)] }

    if ($Target -eq 'c') {
        Write-Host "[3/3] Building omg.exe with $CC"
        & $Omg[0] @OmgArgs --emit-c "$SrcDir/omg.omg" "$Work/omg.c"
        if ($LASTEXITCODE -ne 0) { throw "omg --emit-c failed (exit code $LASTEXITCODE)" }
        & $CC -O3 -w "$Work/omg.c" -o "$Work/omg-new.exe" -static -lm -lws2_32
        if ($LASTEXITCODE -ne 0) { throw "$CC failed (exit code $LASTEXITCODE)" }
    } else {
        Write-Host '[3/3] Building omg.exe'
        # Twice. The first pass is the new source built by whatever
        # `omg` was at hand; the second is the new source built by
        # itself, which is what gets installed.
        New-Item -ItemType Directory -Force -Path "$Work/stage1/x64" | Out-Null
        Copy-Item -Force "$BinDir/x64/*.omg" "$Work/stage1/x64/"
        & $Omg[0] @OmgArgs --build "$SrcDir/omg.omg" "$Work/stage1/omg.exe" --target windows-x64 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "omg --build failed (exit code $LASTEXITCODE)" }
        & "$Work/stage1/omg.exe" --build "$SrcDir/omg.omg" "$Work/omg-new.exe" --target windows-x64 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "the rebuilt omg failed to build itself (exit code $LASTEXITCODE)" }
    }

    # Built under a temporary name and moved into place. By now the
    # omg.exe that started this has exited, so it can be replaced.
    Move-Item -Force "$Work/omg-new.exe" $OmgExe
} finally {
    Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}

Write-Host ''
Write-Host "Native toolchain in ${BinDir}:"
Get-ChildItem $BinDir | Format-Table Name, Length, LastWriteTime -AutoSize | Out-Host
Write-Host 'Run a program with:    bin\omg.exe foo.omg'
Write-Host 'Build an executable:   bin\omg.exe --build foo.omg'
