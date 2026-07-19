$cmPath = "C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI\Services\ClipboardManager.cs"
$cmContent = "using System.Collections.Generic; namespace Deskdrop.WinUI { public class ClipboardManager { public static void Initialize() {} public static void StartMonitoring() {} public static void StopMonitoring() {} public static void SetClipboardFile(string p) {} public static void SetClipboardFiles(List<string> p) {} public static void SetClipboardText(string t) {} } }"
Set-Content $cmPath $cmContent -Encoding UTF8

$dsPath = "C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI\DeskdropStore.cs"
$dsContent = Get-Content $dsPath -Raw
$dsContent = $dsContent -replace '(?s)public class HistoryItem.*?\}', ''
$dsContent = $dsContent + "
namespace Deskdrop.WinUI { public class HistoryItem { public string id {get;set;} public string display_text {get;set;} public string path {get;set;} public bool is_text {get;set;} } }"
Set-Content $dsPath $dsContent -Encoding UTF8
