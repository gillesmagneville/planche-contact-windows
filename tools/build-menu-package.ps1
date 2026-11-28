# SPDX-License-Identifier: GPL-3.0-only
#
# build-menu-package.ps1 - builds the sparse package of the Windows 11 menu
# entry (shell-menu/packaging/) into a folder about to be installed, signed
# with a self-signed certificate made for the occasion:
#
#   DIST\planche-contact-menu.msix   the package (manifest and logos only)
#   DIST\planche-contact-menu.cer    its certificate (public part)
#
# installer\menu-package.ps1 (copied to DIST too) registers it at
# installation. Needs makeappx.exe and signtool.exe of the Windows SDK.
#
# Usage (Windows PowerShell, for its PKI module):
#   powershell -File tools\build-menu-package.ps1 -Dist dist\planche-contact

param(
    [Parameter(Mandatory = $true)] [string] $Dist
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$version = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
# Must match $Publisher of installer\menu-package.ps1.
$publisher = 'CN=Planche-Contact File Explorer menu'

function Find-SdkTool([string] $name) {
    $found = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\$name" -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Directory.Parent.Name } -Descending | Select-Object -First 1
    if ($null -eq $found) { throw "$name not found: install the Windows SDK" }
    return $found.FullName
}
$makeappx = Find-SdkTool 'makeappx.exe'
$signtool = Find-SdkTool 'signtool.exe'

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

# A new self-signed certificate for each build: only its public part is
# shipped; the private key is thrown away once the package is signed.
$cert = New-SelfSignedCertificate -Type Custom -Subject $publisher -KeyUsage DigitalSignature `
    -FriendlyName 'Planche-Contact File Explorer menu' -CertStoreLocation 'Cert:\CurrentUser\My' `
    -NotAfter (Get-Date).AddYears(20) `
    -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
try {
    & $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $package
    if ($LASTEXITCODE -ne 0) { throw "signtool failed ($LASTEXITCODE)" }
    Export-Certificate -Cert $cert -FilePath (Join-Path $Dist 'planche-contact-menu.cer') | Out-Null
} finally {
    Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
}
Copy-Item (Join-Path $root 'installer\menu-package.ps1') $Dist
Write-Host "Built $package (version $version.0), signed by $publisher"
