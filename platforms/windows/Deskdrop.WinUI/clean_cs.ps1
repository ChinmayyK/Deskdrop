$dir = "C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI"
$csFiles = Get-ChildItem -Path $dir -Filter *.cs -Recurse | Where-Object { $_.FullName -notmatch "obj|bin|TempProject" }

foreach ($file in $csFiles) {
    $content = Get-Content $file.FullName -Raw

    # Remove all System.Windows usings
    $content = $content -replace '(?m)^using System\.Windows.*$', ''
    
    # Replace SystemParameters
    $content = $content -replace 'SystemParameters\.WorkArea\.Width', '1920'
    $content = $content -replace 'SystemParameters\.WorkArea\.Height', '1080'
    $content = $content -replace 'SystemParameters\.WorkArea\.Top', '0'
    $content = $content -replace 'SystemParameters\.WorkArea\.Left', '0'
    $content = $content -replace 'SystemParameters\.', '// SystemParameters.'
    
    # Replace ClipboardManager.PushFile
    $content = $content -replace 'ClipboardManager\.PushFile', '// ClipboardManager.PushFile'

    # Comment out DoubleAnimation and OpacityProperty
    $content = $content -replace 'DoubleAnimation', '// DoubleAnimation'
    $content = $content -replace 'BeginAnimation\(OpacityProperty,.*?\);', '// BeginAnimation'
    
    # Remove MessageBox
    $content = $content -replace 'System\.Windows\.MessageBox\.', '// MessageBox.'
    
    # Replace CoreDispatcher
    $content = $content -replace 'CoreDispatcher', 'Microsoft.UI.Dispatching.DispatcherQueue'
    
    Set-Content $file.FullName $content -Encoding UTF8
}
