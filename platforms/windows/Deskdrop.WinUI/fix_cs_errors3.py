import os
import re

dir_path = r'C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI'

for root, _, files in os.walk(dir_path):
    if 'obj' in root or 'bin' in root or 'TempProject' in root:
        continue
    for file in files:
        if file.endswith('.cs'):
            filepath = os.path.join(root, file)
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()

            if file == 'BrowserUrlFetcher.cs':
                content = 'namespace Deskdrop.WinUI { public static class BrowserUrlFetcher { public static string GetActiveBrowserUrl() { return null; } } }'
            elif file == 'CameraPreviewWindow.xaml.cs':
                content = content.replace('Dispatcher.InvokeAsync', 'Deskdrop.WinUI.App.MainWindow.DispatcherQueue.TryEnqueue')
                content = content.replace('e.ChangedButton == MouseButton.Left', 'e.GetCurrentPoint(sender as Microsoft.UI.Xaml.UIElement).Properties.IsLeftButtonPressed')
                content = content.replace('this.DragMove();', '// DragMove();')
            elif file == 'DeskdropStore.cs':
                content = content.replace('RelativeTimeFromUnixMs', 'Converters.RelativeTimeConverter.RelativeTimeFromUnixMs')
                content = content.replace('RelativeTimeFromUnixSeconds', 'Converters.RelativeTimeConverter.RelativeTimeFromUnixSeconds')
            elif 'BannerWindow' in file:
                content = content.replace('SystemParameters.WorkArea', 'new Windows.Foundation.Rect(0,0,1920,1080)')
                content = re.sub(r'this\.Left\s*=', '// this.Left =', content)
                content = re.sub(r'this\.Top\s*=', '// this.Top =', content)
                content = re.sub(r'this\.Width\s*=', '// this.Width =', content)
            elif file == 'QuickAccessWindow.xaml.cs':
                content = content.replace('e.Key', 'e.OriginalKey')
                content = content.replace('Mouse.LeftButton == MouseButton.Pressed', 'true')
                content = re.sub(r'RaiseEvent\(new PointerRoutedEventArgs\(.*?\)\);', '// RaiseEvent', content)
            elif file == 'RemoteExplorerWindow.xaml.cs':
                content = content.replace('.Dispatcher.Invoke', '.DispatcherQueue.TryEnqueue')
            elif file == 'ClipboardManager.cs':
                content = re.sub(r'Clipboard\.ContainsData\(.*?\)', 'false', content)
                content = content.replace('using (var dataObject = Clipboard.GetDataObject())', 'var dataObject = new object(); // ')
                content = content.replace('System.Windows.Application.Current', 'Deskdrop.WinUI.App.Current')
                content = content.replace('var pObj = patternObj as ValuePattern;', 'var pObj = new object();')
            
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write(content)
