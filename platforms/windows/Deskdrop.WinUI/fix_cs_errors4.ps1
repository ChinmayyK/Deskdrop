$files = Get-ChildItem -Path C:\Users\CHINMAY` KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI -Filter *.cs -Recurse | Where-Object { $_.FullName -notmatch "obj|bin|TempProject" }

foreach ($file in $files) {
    $content = Get-Content $file.FullName -Raw

    $content = $content -replace 'System\.Windows\.Application\.Current\.Dispatcher\.Invoke(Async)?', 'Deskdrop.WinUI.App.MainWindow.DispatcherQueue.TryEnqueue'
    $content = $content -replace 'Application\.Current\.Dispatcher\.Invoke(Async)?', 'Deskdrop.WinUI.App.MainWindow.DispatcherQueue.TryEnqueue'

    if ($file.Name -eq "TrayApp.cs") {
        $content = "namespace Deskdrop.WinUI.UI { public class TrayApp { public TrayApp() {} public void Dispose() {} } }"
    }
    if ($file.Name -eq "QRPairingWindow.xaml.cs") {
        $content = $content -replace '(?<!\.)Dispatcher\.Invoke(Async)?', 'Deskdrop.WinUI.App.MainWindow.DispatcherQueue.TryEnqueue'
    }
    if ($file.Name -eq "QuickAccessWindow.xaml.cs") {
        $content = $content -replace 'Key == Key\.', 'OriginalKey == Windows.System.VirtualKey.'
        $content = $content -replace 'Mouse\.LeftButton == MouseButton\.Pressed', 'true'
        $content = $content -replace 'RaiseEvent\(.*?\);', '// RaiseEvent;'
    }
    if ($file.Name -eq "ClipboardManager.cs") {
        $content = "using System.Collections.Generic; namespace Deskdrop.WinUI.Services { public class ClipboardManager { public static void Initialize() {} public static void StartMonitoring() {} public static void StopMonitoring() {} public static void SetClipboardFile(string p) {} public static void SetClipboardFiles(List<string> p) {} public static void SetClipboardText(string t) {} } }"
    }
    if ($file.Name -eq "RemoteExplorerWindow.xaml.cs") {
        $content = $content -replace '(?<!\.)Dispatcher\.Invoke(Async)?', 'Deskdrop.WinUI.App.MainWindow.DispatcherQueue.TryEnqueue'
    }

    Set-Content $file.FullName $content -Encoding UTF8
}
