# Remove only the portable program files next to this script. Never force-stop
# another app: it may have unsaved work or still need to restore a hidden window.
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'omniscreenveil-desktop.exe'
$dll = Join-Path $PSScriptRoot 'veil_hook_win.dll'

Write-Host "OmniScreenVeil portable uninstall: $PSScriptRoot"
Write-Host 'This removes the portable executable and helper DLL. Your app-data folder is kept.'
if ((Read-Host 'Continue? [y/N]') -notmatch '^(y|yes)$') { exit 0 }

$active = Get-Process -Name 'omniscreenveil-desktop' -ErrorAction SilentlyContinue | Where-Object {
    # If access to the path is denied, ask the user to quit rather than risk
    # uninstalling while an elevated copy is still running.
    -not $_.Path -or $_.Path -eq $exe
}
if ($active) {
    Write-Warning 'Use the OmniScreenVeil system tray menu and choose Quit, then run the uninstaller again.'
    exit 1
}

try {
    # Disable login startup only if this copy owns the registration.
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    $runEntry = Get-ItemProperty -LiteralPath $runKey -Name 'OmniScreenVeil' -ErrorAction SilentlyContinue
    $command = if ($runEntry) { ([string]$runEntry.OmniScreenVeil).Trim() } else { '' }
    if ($command -eq $exe -or $command -eq ('"' + $exe + '"')) {
        Remove-ItemProperty -LiteralPath $runKey -Name 'OmniScreenVeil'
    }

    # Remove the helper first. If it is locked, keep the application intact
    # so the user can still launch it to restore protections if needed.
    foreach ($path in @($dll, $exe)) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            Remove-Item -LiteralPath $path -Force
        }
    }
} catch {
    Write-Warning $_.Exception.Message
    Write-Warning 'Quit OmniScreenVeil from the tray and close any apps you protected with Exclude. They can keep the helper DLL loaded after OmniScreenVeil quits.'
    Write-Host 'If the file is still locked, restart Windows and run this uninstaller before reopening those apps.'
    exit 1
}

Write-Host 'Portable program files removed. You can now delete this folder, or keep app-data to preserve your settings.'
