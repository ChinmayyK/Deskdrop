# Deskdrop Windows Setup Script
# This script ensures the .NET SDK is available, copies the native backend DLL,
# publishes the WPF app as a standalone release, and places a shortcut on your Desktop.

Write-Host "========================================"
Write-Host " Deskdrop Windows & Android Setup "
Write-Host "========================================"

# 0. Clean up previous installations
Write-Host "[*] Cleaning up previous Windows installation..." -ForegroundColor Cyan
Stop-Process -Name "Deskdrop" -Force -ErrorAction SilentlyContinue
$installPath = "$env:LOCALAPPDATA\DeskdropApp"
if (Test-Path $installPath) {
    Remove-Item -Path $installPath -Recurse -Force -ErrorAction SilentlyContinue
}
$legacyPath = "$env:LOCALAPPDATA\Programs\Deskdrop"
if (Test-Path $legacyPath) {
    Remove-Item -Path $legacyPath -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "[*] Cleaning up previous Android installation..." -ForegroundColor Cyan
if (Get-Command "adb" -ErrorAction SilentlyContinue) {
    adb uninstall com.deskdrop | Out-Null
} else {
    Write-Host "[-] adb not found in PATH. Skipping Android uninstall." -ForegroundColor Yellow
}

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
$dotnetCmd = "$env:LOCALAPPDATA\Microsoft\dotnet\dotnet.exe"
if (-not (Test-Path $dotnetCmd)) {
    $dotnetCmd = "dotnet"
}
Write-Host "[+] Using .NET SDK at $dotnetCmd"

# 3. Publish the Application
$installPath = "$env:LOCALAPPDATA\DeskdropApp"
Write-Host "[*] Building and publishing Deskdrop to $installPath..." -ForegroundColor Cyan

& $dotnetCmd publish Deskdrop.WinUI.csproj -c Release -r win-x64 --self-contained true -o $installPath

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

# 5. Launch Windows Application
Write-Host "[*] Launching Deskdrop for Windows..." -ForegroundColor Cyan
Start-Process "$installPath\Deskdrop.exe"

# 6. Install Android Application
Write-Host "[*] Building and installing Android application..." -ForegroundColor Cyan
$androidDir = "..\..\android"
if (Test-Path "$androidDir\gradlew.bat") {
    Push-Location $androidDir
    Write-Host "[*] Running Gradle installDebug..."
    .\gradlew.bat installDebug
    if ($LASTEXITCODE -eq 0) {
        Write-Host "[+] Android app installed successfully!" -ForegroundColor Green
    } else {
        Write-Host "[-] Android app installation failed." -ForegroundColor Red
    }
    Pop-Location
} else {
    Write-Host "[-] Android project not found at $androidDir." -ForegroundColor Yellow
}

Write-Host "========================================"
Write-Host " Setup Complete! Deskdrop is installed on both platforms." -ForegroundColor Green
Write-Host "========================================"
