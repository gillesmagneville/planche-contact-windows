# SPDX-License-Identifier: GPL-3.0-only
#
# check-install.ps1 - installs Planche-Contact with its installer, checks
# what was installed, starts the installed window, then uninstalls and
# checks that nothing is left. Used by the continuous integration, on a
# Windows where Planche-Contact is not installed.
#
# Usage: pwsh tools/check-install.ps1 -Installer dist\planche-contact-X.Y.Z-setup.exe
#
# Exit code 0 when every check passed; otherwise the failed check is
# printed.

param(
    [Parameter(Mandatory = $true)] [string] $Installer
)

$ErrorActionPreference = 'Stop'
# The Appx cmdlets come from Windows PowerShell.
if ($PSVersionTable.PSEdition -eq 'Core') { Import-Module Appx -UseWindowsPowerShell -WarningAction SilentlyContinue }

$version = (Get-Content (Join-Path $PSScriptRoot '..\VERSION') -Raw).Trim()
$dir = Join-Path $env:LOCALAPPDATA 'Programs\Planche-Contact'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Planche-Contact'
# shell-verb/src/plan.rs: CLSID and VERB_KEY.
$classKey = 'HKCU:\Software\Classes\CLSID\{8BA32AF5-D148-4EEE-8021-9DF550039E2F}'
$verbKeys = @(
    'HKCU:\Software\Classes\Directory\shell\PlancheContact',
    'HKCU:\Software\Classes\Directory\Background\shell\PlancheContact',
    'HKCU:\Software\Classes\SystemFileAssociations\.jpg\shell\PlancheContact',
    'HKCU:\Software\Classes\SystemFileAssociations\.nef\shell\PlancheContact'
)
# The Windows 11 entry: shell-menu/src/lib.rs (PACKAGE_NAME). Expected when the build holds its
# package and Windows is Windows 11 (or Server 2025); then the classic entry
# is not registered.
$packageName = 'PlancheContact.ExplorerMenu'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Planche-Contact.lnk'
$crashLog = Join-Path $env:TEMP 'planche-contact-app.log'

function Fail([string] $message) {
    Write-Host "FAILED: $message"
    exit 1
}

function Default-Value([string] $key) {
    (Get-Item -LiteralPath $key).GetValue('')
}

if (Test-Path $dir) { Fail "$dir already exists: run this on a machine without Planche-Contact" }
Remove-Item $crashLog -ErrorAction SilentlyContinue

# ------------------------------------------------------------- install
$run = Start-Process -FilePath (Resolve-Path $Installer).Path -ArgumentList '/S' -Wait -PassThru
if ($run.ExitCode -ne 0) { Fail "the installer ended with exit code $($run.ExitCode)" }

foreach ($file in 'PlancheContact.exe', 'PlancheContact.pri', 'engine\planche-contact-engine.exe',
                  'planche-contact-verb.exe', 'Uninstall.exe', 'docs\planche-contact-manual.html',
                  'docs\planche-contact-manual.en.html', 'docs\planche-contact-manual.de.html',
                  'docs\planche-contact-manual.es.html') {
    if (-not (Test-Path (Join-Path $dir $file))) { Fail "not installed: $file" }
}
Write-Host "ok: the programs are in $dir"

$entry = Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction SilentlyContinue
if ($null -eq $entry) { Fail 'no entry in Settings > Apps' }
if ($entry.DisplayVersion -ne $version) { Fail "Settings > Apps shows version '$($entry.DisplayVersion)', not $version" }
if ($entry.EstimatedSize -lt 1000) { Fail "implausible installed size: $($entry.EstimatedSize) KB" }
Write-Host "ok: Settings > Apps shows Planche-Contact $version ($([int]($entry.EstimatedSize / 1024)) MB)"

$firstLevel = ([Environment]::OSVersion.Version.Build -ge 22000) -and (Test-Path (Join-Path $dir 'planche-contact-menu.msix'))
if ($firstLevel) {
    $package = Get-AppxPackage -Name $packageName
    if ($null -eq $package) {
        # What menu-package.ps1 reported, and what the deployment service says.
        Get-Content (Join-Path $env:TEMP 'planche-contact-verb.log') -ErrorAction SilentlyContinue
        Get-AppxLog -ErrorAction SilentlyContinue | Select-Object -Last 15 | Format-List TimeCreated, Message
        Fail "the Windows 11 menu package $packageName is not registered"
    }
    if ($package.Version -ne "$version.0") { Fail "the menu package has version $($package.Version), not $version.0" }
    if (Test-Path -LiteralPath $classKey) { Fail 'the classic menu handler is registered too (two entries)' }
    Write-Host "ok: the Windows 11 menu package is registered ($($package.PackageFullName))"
} else {
    $server = Default-Value "$classKey\LocalServer32"
    if ($server -ne "`"$dir\planche-contact-verb.exe`"") { Fail "the menu handler is registered as '$server'" }
    foreach ($key in $verbKeys) {
        if (-not (Test-Path -LiteralPath $key)) { Fail "no menu entry under $key" }
        $label = Default-Value $key
        if ($label -notmatch 'Planche-Contact') { Fail "unexpected menu label '$label' under $key" }
    }
    Write-Host "ok: the File Explorer menu entry is registered ('$(Default-Value $verbKeys[0])')"
}

if (-not (Test-Path $shortcut)) { Fail "no Start menu shortcut ($shortcut)" }
Write-Host 'ok: Start menu shortcut'

$engineVersion = & (Join-Path $dir 'engine\planche-contact-engine.exe') --version
if ($LASTEXITCODE -ne 0 -or "$engineVersion" -notmatch [regex]::Escape($version)) {
    Fail "the installed engine answers '$engineVersion'"
}
Write-Host "ok: the installed engine runs ($engineVersion)"

# The installed window, as the Start menu shortcut starts it.
$window = Start-Process -FilePath (Join-Path $dir 'PlancheContact.exe') -PassThru
Start-Sleep -Seconds 20
if ($window.HasExited) { Fail "the installed window stopped by itself (exit code $($window.ExitCode))" }
Stop-Process -Id $window.Id -Force
$window.WaitForExit()
if ((Test-Path $crashLog) -and (Get-Content $crashLog -Raw).Trim()) {
    Get-Content $crashLog
    Fail 'the installed window logged an unhandled error'
}
Write-Host 'ok: the installed window starts and stays up'

# ----------------------------------------------------------- uninstall
# The NSIS uninstaller copies itself to the temporary folder and returns at
# once: wait for the folder to go.
Start-Process -FilePath (Join-Path $dir 'Uninstall.exe') -ArgumentList '/S' -Wait | Out-Null
$deadline = (Get-Date).AddSeconds(90)
while ((Test-Path $dir) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
if (Test-Path $dir) { Fail "$dir is still there after uninstalling" }
if (Test-Path -LiteralPath $uninstallKey) { Fail 'the entry in Settings > Apps is still there' }
if (Test-Path -LiteralPath $classKey) { Fail 'the menu handler is still registered' }
foreach ($key in $verbKeys) {
    if (Test-Path -LiteralPath $key) { Fail "the menu entry under $key is still there" }
}
if (Test-Path $shortcut) { Fail 'the Start menu shortcut is still there' }
if (Get-AppxPackage -Name $packageName) { Fail 'the Windows 11 menu package is still registered' }
Write-Host 'ok: uninstalling removes the programs, the menu entry, the shortcut and the Settings entry'

Write-Host 'All checks passed.'
