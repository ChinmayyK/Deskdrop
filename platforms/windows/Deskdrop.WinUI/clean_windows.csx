using System.IO;
using System.Text.RegularExpressions;

var dir = @"C:\Users\CHINMAY KUDALKAR\.gemini\antigravity\scratch\Deskdrop\platforms\windows\Deskdrop.WinUI";

var xamlFiles = Directory.GetFiles(dir, "*.xaml", SearchOption.AllDirectories);

foreach (var xamlFile in xamlFiles)
{
    if (xamlFile.Contains("obj") || xamlFile.Contains("bin") || xamlFile.Contains("App.xaml") || xamlFile.Contains("MainWindow.xaml") || xamlFile.Contains("Themes")) continue;
    
    var name = Path.GetFileNameWithoutExtension(xamlFile);
    
    // Rewrite XAML
    var xamlContent = $@"<Window
    x:Class="Deskdrop.WinUI.{name}"
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
    <Grid></Grid>
</Window>";
    
    // Deal with subfolders for namespace
    var ns = "Deskdrop.WinUI";
    if (xamlFile.Contains("\\Views\\")) ns = "Deskdrop.WinUI.Views";
    if (xamlFile.Contains("\\UI\\")) ns = "Deskdrop.WinUI.UI";
    
    xamlContent = $@"<Window
    x:Class="{ns}.{name}"
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
    <Grid></Grid>
</Window>";

    File.WriteAllText(xamlFile, xamlContent);
    
    // Rewrite CS
    var csFile = xamlFile + ".cs";
    if (File.Exists(csFile)) {
        var csContent = $@"using Microsoft.UI.Xaml;
namespace {ns}
{{
    public partial class {name} : Window
    {{
        public {name}()
        {{
            this.InitializeComponent();
        }}
    }}
}}";
        File.WriteAllText(csFile, csContent);
    }
}
