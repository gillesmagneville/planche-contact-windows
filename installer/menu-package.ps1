# SPDX-License-Identifier: GPL-3.0-only
#
# menu-package.ps1 - puts "Open in Planche-Contact" on the first level of the
# Windows 11 File Explorer menu, or takes it off. Installed next to the
# programs and run by the installer and the uninstaller, for the current
# user, without administrator rights.
#
#   menu-package.ps1 -Register [-RestartExplorer]
#       Registers the sparse package planche-contact-menu.msix with this
#       folder as its external location. Exit code 0: registered; 2: not
#       applicable (Windows 10, or a build without the package); 1: failed.
#       With -RestartExplorer, File Explorer is restarted so that the entry
#       shows at once.
#   menu-package.ps1 -Unregister
#       Removes the package.
#
# The package is unsigned: Windows 11 accepts an unsigned package whose
# publisher carries Microsoft's "unsigned" identifier (-AllowUnsigned). A
# self-signed one would need its certificate in the machine's trusted
# store, hence administrator rights; the current user's store is not
# enough.
#
# Failures are appended to planche-contact-verb.log in %TEMP%.

param(
    [switch] $Register,
    [switch] $Unregister,
    [switch] $RestartExplorer
)

$ErrorActionPreference = 'Stop'
# shell-menu/src/lib.rs: PACKAGE_NAME.
$PackageName = 'PlancheContact.ExplorerMenu'
$dir = $PSScriptRoot
$package = Join-Path $dir 'planche-contact-menu.msix'

function Log([string] $message) {
    $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    Add-Content -Path (Join-Path $env:TEMP 'planche-contact-verb.log') -Value "[$stamp] menu-package.ps1: $message" -ErrorAction SilentlyContinue
}

try {
    if ($Unregister) {
        Get-AppxPackage -Name $PackageName | Remove-AppxPackage
        exit 0
    }
    if (-not $Register) { throw 'expected -Register or -Unregister' }

    if ([Environment]::OSVersion.Version.Build -lt 22000) { exit 2 }   # the new menu is Windows 11's
    if (-not (Test-Path $package)) { exit 2 }

    Get-AppxPackage -Name $PackageName | Remove-AppxPackage   # an earlier version
    Add-AppxPackage -Path $package -ExternalLocation $dir -AllowUnsigned
    if ($null -eq (Get-AppxPackage -Name $PackageName)) { throw 'the package is not registered after Add-AppxPackage' }

    if ($RestartExplorer) {
        # Windows starts the shell again by itself; start it if it did not.
        Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3
        if ($null -eq (Get-Process -Name explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }
    }
    exit 0
} catch {
    Log $_.Exception.Message
    exit 1
}
