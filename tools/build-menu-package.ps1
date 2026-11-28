# SPDX-License-Identifier: GPL-3.0-only
#
# build-menu-package.ps1 - builds the sparse package of the Windows 11 menu
# entry (shell-menu/packaging/) into a folder about to be installed, as
# DIST\planche-contact-menu.msix: its manifest and logos only, unsigned.
#
# installer\menu-package.ps1 (copied to DIST too) registers it at
# installation. Needs makeappx.exe of the Windows SDK.
#
# Usage:
#   powershell -File tools\build-menu-package.ps1 -Dist dist\planche-contact

param(
    [Parameter(Mandatory = $true)] [string] $Dist
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
# The OID marks an unsigned package, which Windows 11 installs with
# Add-AppxPackage -AllowUnsigned (installer\menu-package.ps1): no
# certificate to trust, hence no administrator rights.
$publisher = 'CN=Planche-Contact, OID.2.25.311729368913984317654407730594956997722=1'

function Find-SdkTool([string] $name) {
    $found = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\$name" -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Directory.Parent.Name } -Descending | Select-Object -First 1
    if ($null -eq $found) { throw "$name not found: install the Windows SDK" }
    return $found.FullName
}
$makeappx = Find-SdkTool 'makeappx.exe'

if (-not (Test-Path (Join-Path $Dist 'planche_contact_menu.dll'))) {
    throw "$Dist does not hold planche_contact_menu.dll"
}

# The package content: the manifest, with its values, and the logos.
$stage = Join-Path ([IO.Path]::GetTempPath()) "planche-contact-menu-$PID"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item -Recurse (Join-Path $root 'shell-menu\packaging\Assets') $stage
$manifest = Get-Content (Join-Path $root 'shell-menu\packaging\AppxManifest.template.xml') -Raw -Encoding UTF8
$manifest = $manifest.Replace('@VERSION@', "$version.0").Replace('@PUBLISHER@', $publisher)
[IO.File]::WriteAllText((Join-Path $stage 'AppxManifest.xml'), $manifest, (New-Object Text.UTF8Encoding $false))

$package = Join-Path $Dist 'planche-contact-menu.msix'
& $makeappx pack /o /nv /d $stage /p $package
if ($LASTEXITCODE -ne 0) { throw "makeappx failed ($LASTEXITCODE)" }

Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
Copy-Item (Join-Path $root 'installer\menu-package.ps1') $Dist
Write-Host "Built $package (version $version.0, unsigned)"
