$dir = "C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI"
$xamlFiles = Get-ChildItem -Path $dir -Filter *.xaml -Recurse | Where-Object { $_.FullName -notmatch "obj|bin|App\.xaml|MainWindow\.xaml|Themes" }

foreach ($file in $xamlFiles) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($file.Name)
    
    $ns = "Deskdrop.WinUI"
    if ($file.FullName -match "\\Views\\") { $ns = "Deskdrop.WinUI.Views" }
    if ($file.FullName -match "\\UI\\") { $ns = "Deskdrop.WinUI.UI" }
    
    $xamlContent = "<Window
    x:Class="."
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
    <Grid></Grid>
</Window>"

    Set-Content $file.FullName $xamlContent -Encoding UTF8
    
    $csFile = $file.FullName + ".cs"
    if (Test-Path $csFile) {
        $csContent = "using Microsoft.UI.Xaml;
namespace 
{
    public partial class  : Window
    {
        public ()
        {
            this.InitializeComponent();
        }
    }
}"
        Set-Content $csFile $csContent -Encoding UTF8
    }
}
