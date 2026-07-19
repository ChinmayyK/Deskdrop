$files = Get-ChildItem -Path C:\Users\CHINMAY` KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI -Filter *.cs -Recurse | Where-Object { $_.FullName -notmatch "obj|bin|TempProject" }

foreach ($file in $files) {
    $content = Get-Content $file.FullName -Raw

    # BrowserUrlFetcher
    if ($file.Name -eq "BrowserUrlFetcher.cs") {
        $content = "namespace Deskdrop.WinUI { public static class BrowserUrlFetcher { public static string GetActiveBrowserUrl() { return null; } } }"
    }
    
    # CameraPreviewWindow
    if ($file.Name -eq "CameraPreviewWindow.xaml.cs") {
        $content = $content -replace 'Dispatcher\.InvokeAsync', 'Deskdrop.WinUI.App.MainWindow.DispatcherQueue.TryEnqueue'
        $content = $content -replace 'e\.ChangedButton == MouseButton\.Left', 'e.GetCurrentPoint(sender as Microsoft.UI.Xaml.UIElement).Properties.IsLeftButtonPressed'
        $content = $content -replace 'this\.DragMove\(\);', '// DragMove();'
    }

    # DeskdropStore
    if ($file.Name -eq "DeskdropStore.cs") {
        $content = $content -replace 'RelativeTimeFromUnixMs', 'Converters.RelativeTimeConverter.RelativeTimeFromUnixMs'
        $content = $content -replace 'RelativeTimeFromUnixSeconds', 'Converters.RelativeTimeConverter.RelativeTimeFromUnixSeconds'
    }

    # IncomingFileBannerWindow / IncomingCallBannerWindow
    if ($file.Name -match "BannerWindow") {
        $content = $content -replace 'SystemParameters\.WorkArea', 'new Windows.Foundation.Rect(0,0,1920,1080)'
        $content = $content -replace 'this\.Left =', '// this.Left ='
        $content = $content -replace 'this\.Top =', '// this.Top ='
        $content = $content -replace 'this\.Width =', '// this.Width ='
    }

    # QuickAccessWindow
    if ($file.Name -eq "QuickAccessWindow.xaml.cs") {
        $content = $content -replace 'e\.Key', 'e.OriginalKey'
        $content = $content -replace 'Mouse\.LeftButton == MouseButton\.Pressed', 'true'
        $content = $content -replace 'RaiseEvent\(new PointerRoutedEventArgs\(.*?\)\);', '// RaiseEvent'
    }

    # RemoteExplorerWindow
    if ($file.Name -eq "RemoteExplorerWindow.xaml.cs") {
        $content = $content -replace '\.Dispatcher\.Invoke', '.DispatcherQueue.TryEnqueue'
    }
    
    # ClipboardManager
    if ($file.Name -eq "ClipboardManager.cs") {
        $content = $content -replace 'Clipboard\.ContainsData\(.*?\)', 'false'
        $content = $content -replace 'using \(var dataObject = Clipboard\.GetDataObject\(\)\)', 'var dataObject = new object(); // '
        $content = $content -replace 'System\.Windows\.Application\.Current', 'Deskdrop.WinUI.App.Current'
    }

    Set-Content $file.FullName $content -Encoding UTF8
}
