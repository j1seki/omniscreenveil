# Remove the portable package without closing other apps.
param([switch]$ScheduleHelperRemoval, [switch]$RemoveEmptyFolder)

$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'omniscreenveil-desktop.exe'
$dll = Join-Path $PSScriptRoot 'veil_hook_win.dll'
$launcher = Join-Path $PSScriptRoot 'Uninstall-OmniScreenVeil.cmd'
$data = Join-Path $PSScriptRoot 'app-data'
$temp = [System.IO.Path]::GetTempPath()
Set-Location -LiteralPath $temp
[Environment]::CurrentDirectory = $temp

function Register-HelperRemoval {
    param([switch]$IncludeFolder)

    Add-Type -TypeDefinition @'
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class OmniScreenVeilUninstall {
    private const uint DelayUntilReboot = 4;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true, ExactSpelling = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool MoveFileExW(string existingPath, string newPath, uint flags);

    public static void DeleteOnRestart(string path) {
        if (!MoveFileExW(path, null, DelayUntilReboot)) {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }
}
'@
    if (Test-Path -LiteralPath $dll -PathType Leaf) {
        [OmniScreenVeilUninstall]::DeleteOnRestart($dll)
    }
    if ($IncludeFolder) {
        [OmniScreenVeilUninstall]::DeleteOnRestart($PSScriptRoot)
    }
}

function Remove-AppData {
    param([System.IO.FileSystemInfo]$Item)

    if ($Item.PSIsContainer) {
        if (-not ($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
            Get-ChildItem -LiteralPath $Item.FullName -Force | ForEach-Object { Remove-AppData -Item $_ }
        }
        [System.IO.Directory]::Delete($Item.FullName)
    } else {
        Remove-Item -LiteralPath $Item.FullName -Force
    }
}

if ($ScheduleHelperRemoval) {
    try {
        Register-HelperRemoval -IncludeFolder:$RemoveEmptyFolder
        exit 0
    } catch {
        Write-Warning $_.Exception.Message
        exit 1
    }
}

Write-Host "Uninstall OmniScreenVeil from $PSScriptRoot"
Write-Host 'This removes the portable program files and uninstaller.'
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

$deleteData = (Test-Path -LiteralPath $data) -and ((Read-Host 'Delete app-data, including saved settings and logs? [y/N]') -match '^(y|yes)$')
$filesToRemove = @($exe, $dll, $launcher, $PSCommandPath)
if ($deleteData) { $filesToRemove += $data }
$removeFolder = -not (Get-ChildItem -LiteralPath $PSScriptRoot -Force | Where-Object { $_.FullName -notin $filesToRemove } | Select-Object -First 1)

$restartRequired = $false
try {
    # Keep the executable available if the user needs to retry cleanup.
    if (Test-Path -LiteralPath $dll -PathType Leaf) {
        try {
            Remove-Item -LiteralPath $dll -Force
        } catch [System.IO.IOException], [System.UnauthorizedAccessException] {
            if (Test-Path -LiteralPath $dll -PathType Leaf) {
                Write-Warning 'Windows could not delete the helper DLL. A protected app may still have it loaded.'
                if ((Read-Host 'Queue helper removal for the next Windows restart? [y/N]') -notmatch '^(y|yes)$') {
                    Write-Host 'Restart Windows and run the uninstaller again to finish removal.'
                    exit 1
                }

                $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
                try {
                    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
                    $isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
                } finally {
                    $identity.Dispose()
                }
                if ($isAdmin) {
                    Register-HelperRemoval -IncludeFolder:$removeFolder
                } else {
                    Write-Host 'Approve the Windows administrator prompt to schedule removal.'
                    $arguments = '-NoLogo -NoProfile -ExecutionPolicy Bypass -File "{0}" -ScheduleHelperRemoval' -f $PSCommandPath
                    if ($removeFolder) { $arguments += ' -RemoveEmptyFolder' }
                    $powershell = Join-Path ([Environment]::SystemDirectory) 'WindowsPowerShell\v1.0\powershell.exe'
                    $scheduler = Start-Process -FilePath $powershell -ArgumentList $arguments -WorkingDirectory $temp -Verb RunAs -Wait -PassThru
                    if ($scheduler.ExitCode -ne 0) { throw 'Windows could not schedule helper removal.' }
                }
                $restartRequired = Test-Path -LiteralPath $dll -PathType Leaf
            }
        }
    }

    # Disable login startup only if this copy owns the registration.
    $runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    $runEntry = Get-ItemProperty -LiteralPath $runKey -Name 'OmniScreenVeil' -ErrorAction SilentlyContinue
    $command = if ($runEntry) { ([string]$runEntry.OmniScreenVeil).Trim() } else { '' }
    if ($command -eq $exe -or $command -eq ('"' + $exe + '"')) {
        Remove-ItemProperty -LiteralPath $runKey -Name 'OmniScreenVeil'
    }

    if (Test-Path -LiteralPath $exe -PathType Leaf) {
        Remove-Item -LiteralPath $exe -Force
    }
    if ($deleteData -and (Test-Path -LiteralPath $data)) {
        Remove-AppData -Item (Get-Item -LiteralPath $data -Force)
    }
    if (Test-Path -LiteralPath $launcher -PathType Leaf) {
        Remove-Item -LiteralPath $launcher -Force
    }
    Remove-Item -LiteralPath $PSCommandPath -Force
    if (-not $restartRequired -and -not (Get-ChildItem -LiteralPath $PSScriptRoot -Force | Select-Object -First 1)) {
        [System.IO.Directory]::Delete($PSScriptRoot)
    }
} catch {
    Write-Warning $_.Exception.Message
    Write-Host 'Restart Windows and run the uninstaller again if any program files remain.'
    exit 1
}

if ($restartRequired) {
    Write-Host 'The executable and uninstall scripts were removed. Windows will delete the helper DLL on the next restart.'
    if ($removeFolder) { Write-Host 'Windows will also remove the portable folder if it is empty.' }
    Write-Host 'Restart Windows before reusing this portable folder.'
} else {
    Write-Host 'Portable program files removed.'
}
if (Test-Path -LiteralPath $data) { Write-Host 'Your app-data folder was kept.' }
