# SPDX-License-Identifier: GPL-3.0-only
#
# menu-package.ps1 - puts "Open in Planche-Contact" on the first level of the
# Windows 11 File Explorer menu, or takes it off. Installed next to the
# programs and run by the installer and the uninstaller, for the current
# user, without administrator rights.
#
#   menu-package.ps1 -Register [-RestartExplorer]
#       Trusts planche-contact-menu.cer (current user's "Trusted People"),
#       then registers the sparse package planche-contact-menu.msix with this
#       folder as its external location. Exit code 0: registered; 2: not
#       applicable (Windows 10, or a build without the package); 1: failed.
#       With -RestartExplorer, File Explorer is restarted so that the entry
#       shows at once.
#   menu-package.ps1 -Unregister
#       Removes the package and the certificate.
#
# Failures are appended to planche-contact-verb.log in %TEMP%.

param(
    [switch] $Register,
    [switch] $Unregister,
    [switch] $RestartExplorer
)

$ErrorActionPreference = 'Stop'
# shell-menu/src/lib.rs: PACKAGE_NAME; tools/build-menu-package.ps1: $publisher.
$PackageName = 'PlancheContact.ExplorerMenu'
$Publisher = 'CN=Planche-Contact File Explorer menu'
$dir = $PSScriptRoot
$package = Join-Path $dir 'planche-contact-menu.msix'
$certificate = Join-Path $dir 'planche-contact-menu.cer'

# The current user's "Trusted People" store, through .NET: the Cert: drive
# of Windows PowerShell only exists once its module is loaded.
function Open-TrustedPeople {
    $store = New-Object Security.Cryptography.X509Certificates.X509Store 'TrustedPeople', 'CurrentUser'
    $store.Open('ReadWrite')
    return $store
}

function Log([string] $message) {
    $stamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    Add-Content -Path (Join-Path $env:TEMP 'planche-contact-verb.log') -Value "[$stamp] menu-package.ps1: $message" -ErrorAction SilentlyContinue
}

function Remove-Package {
    Get-AppxPackage -Name $PackageName | Remove-AppxPackage
    $store = Open-TrustedPeople
    try {
        foreach ($cert in @($store.Certificates | Where-Object { $_.Subject -eq $Publisher })) { $store.Remove($cert) }
    } finally { $store.Close() }
}

try {
    if ($Unregister) {
        Remove-Package
        exit 0
    }
    if (-not $Register) { throw 'expected -Register or -Unregister' }

    if ([Environment]::OSVersion.Version.Build -lt 22000) { exit 2 }   # the new menu is Windows 11's
    if (-not (Test-Path $package) -or -not (Test-Path $certificate)) { exit 2 }

    Remove-Package   # an earlier version, and its certificate
    $store = Open-TrustedPeople
    try {
        $store.Add((New-Object Security.Cryptography.X509Certificates.X509Certificate2 $certificate))
    } finally { $store.Close() }
    Add-AppxPackage -Path $package -ExternalLocation $dir
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
