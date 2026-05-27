# Deskdrop Windows Setup Script
# This script ensures the .NET SDK is available, copies the native backend DLL,
# publishes the WPF app as a standalone release, and places a shortcut on your Desktop.

Write-Host "========================================"
Write-Host " Deskdrop Windows Setup & Install"
Write-Host "========================================"

# 1. Ensure the native DLL is in the correct target directory
$targetDir = "..\..\..\target\release"
if (-not (Test-Path $targetDir)) {
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
}

$dllSource = "..\..\..\release\windows\deskdrop_core.dll"
if (Test-Path $dllSource) {
    Copy-Item $dllSource -Destination $targetDir -Force
    Write-Host "[+] Native backend linked successfully."
} else {
    Write-Host "[!] Warning: Could not find pre-built deskdrop_core.dll in release/windows." -ForegroundColor Yellow
}

# 2. Check for .NET SDK
$dotnetCmd = "dotnet"
if (-not (Get-Command "dotnet" -ErrorAction SilentlyContinue)) {
    $localDotnet = "$env:LOCALAPPDATA\Microsoft\dotnet\dotnet.exe"
    if (Test-Path $localDotnet) {
        $dotnetCmd = $localDotnet
        Write-Host "[+] Found local .NET SDK installation."
    } else {
        Write-Host "[-] .NET SDK not found. Installing .NET 8 SDK automatically..." -ForegroundColor Cyan
        Invoke-WebRequest -Uri "https://dot.net/v1/dotnet-install.ps1" -OutFile "dotnet-install.ps1"
        powershell -ExecutionPolicy Bypass -File .\dotnet-install.ps1 -Channel 8.0
        $dotnetCmd = $localDotnet
    }
} else {
    Write-Host "[+] .NET SDK found in system PATH."
}

# 3. Publish the Application
$installPath = "$env:LOCALAPPDATA\DeskdropApp"
Write-Host "[*] Building and publishing Deskdrop to $installPath..." -ForegroundColor Cyan

& $dotnetCmd publish Deskdrop.Windows.csproj -c Release -r win-x64 --self-contained true -o $installPath

if ($LASTEXITCODE -ne 0) {
    Write-Host "[-] Build failed. Please check the errors above." -ForegroundColor Red
    Exit
}

Write-Host "[+] Build published successfully!" -ForegroundColor Green

# 4. Create Desktop Shortcut
$WshShell = New-Object -comObject WScript.Shell
$ShortcutPath = "$env:USERPROFILE\Desktop\Deskdrop.lnk"
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = "$installPath\Deskdrop.exe"
$Shortcut.WorkingDirectory = $installPath
$Shortcut.IconLocation = "$installPath\Deskdrop.exe,0"
$Shortcut.Save()

Write-Host "[+] Desktop shortcut created at $ShortcutPath."

# 5. Launch Application
Write-Host "[*] Launching Deskdrop..." -ForegroundColor Cyan
Start-Process "$installPath\Deskdrop.exe"

Write-Host "========================================"
Write-Host " Setup Complete! You can now launch Deskdrop from your Desktop." -ForegroundColor Green
Write-Host "========================================"
