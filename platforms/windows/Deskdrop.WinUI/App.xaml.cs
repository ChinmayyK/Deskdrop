using Microsoft.UI.Dispatching;
using Windows.ApplicationModel;
using Windows.ApplicationModel.Activation;
using Windows.Foundation;
using Windows.Foundation.Collections;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;
using System;

namespace Deskdrop.WinUI;

public partial class App : Application
{
    public static Window MainWindow { get; private set; }
    private Window? _window;
    
    // Tray Icon Properties
    public H.NotifyIcon.TaskbarIcon? TrayIcon { get; private set; }
    
    // Commands
    public System.Windows.Input.ICommand ShowMainWindowCommand { get; }
    public System.Windows.Input.ICommand ExitApplicationCommand { get; }

    public App()
    {
        InitializeComponent();
        
        ShowMainWindowCommand = new RelayCommand(() =>
        {
            if (MainWindow == null)
            {
                MainWindow = new MainWindow();
            }
            MainWindow.Activate();
        });
        
        ExitApplicationCommand = new RelayCommand(() =>
        {
            TrayIcon?.Dispose();
            Application.Current.Exit();
        });
    }

    protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
    {
        // Initialize the Tray Icon entirely in C# to avoid x:Bind issues in App.xaml
        TrayIcon = new H.NotifyIcon.TaskbarIcon
        {
            ToolTipText = "Deskdrop",
            IconSource = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri("ms-appx:///Assets/AppIcon.ico")),
            LeftClickCommand = ShowMainWindowCommand
        };

        var menu = new MenuFlyout();
        var openItem = new MenuFlyoutItem { Text = "Open Deskdrop", Command = ShowMainWindowCommand };
        openItem.Icon = new FontIcon { Glyph = "\uE8A7" };
        var quitItem = new MenuFlyoutItem { Text = "Quit", Command = ExitApplicationCommand };
        quitItem.Icon = new FontIcon { Glyph = "\uE711" };

        menu.Items.Add(openItem);
        menu.Items.Add(new MenuFlyoutSeparator());
        menu.Items.Add(quitItem);

        TrayIcon.ContextFlyout = menu;
        TrayIcon.ForceCreate();
        
        MainWindow = new MainWindow();
        _window = MainWindow;
        _window.Activate();
    }
}

public class RelayCommand : System.Windows.Input.ICommand
{
    private readonly Action _execute;
    public RelayCommand(Action execute) => _execute = execute;
    public event EventHandler? CanExecuteChanged;
    public bool CanExecute(object? parameter) => true;
    public void Execute(object? parameter) => _execute();
}
