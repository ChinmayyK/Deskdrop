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

    private static System.Threading.Mutex? _singleInstanceMutex;
    private static IntPtr _engineHandle = IntPtr.Zero;
    public static IntPtr EngineHandle => _engineHandle;
    public static Deskdrop.WinUI.Services.ClipboardManager? Clipboard { get; private set; }

    public App()
    {
        var dir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
        
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App constructor started\n");

        InitializeComponent();
        
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App InitializeComponent finished\n");
        
        ShowMainWindowCommand = new RelayCommand(() =>
        {
            if (MainWindow == null)
            {
                MainWindow = new DashboardWindow();
            }
            MainWindow.Activate();
        });
        
        ExitApplicationCommand = new RelayCommand(() =>
        {
            TrayIcon?.Dispose();
            if (_engineHandle != IntPtr.Zero)
            {
                try { NativeCore.deskdrop_stop(_engineHandle); _engineHandle = IntPtr.Zero; } catch { }
            }
            try { GlobalHotKeyManager.Shared.Dispose(); } catch { }
            Application.Current.Exit();
        });
    }

    protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
    {
        var dir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] OnLaunched started\n");

        try
        {
            if (!DaemonClient.IsDaemonRunning())
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Starting native engine via deskdrop_start...\n");
                _engineHandle = NativeCore.deskdrop_start(null, 0);
                if (_engineHandle == IntPtr.Zero)
                {
                    System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] ERROR: deskdrop_start returned null handle!\n");
                }
            }

            Clipboard = new Deskdrop.WinUI.Services.ClipboardManager();

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
            
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] TrayIcon created. Creating MainWindow...\n");

            MainWindow = new DashboardWindow();
            _window = MainWindow;
            _window.Activate();
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] MainWindow activated.\n");

            try
            {
                GlobalHotKeyManager.Shared.Register(true, true, false, false, 0x56, () => {
                    _window?.DispatcherQueue.TryEnqueue(() => new QuickAccessWindow().Activate());
                });
                GlobalHotKeyManager.Shared.Register(true, false, false, false, 0x4B, () => {
                    _window?.DispatcherQueue.TryEnqueue(() => {
                        if (_window == null) _window = new DashboardWindow();
                        _window.Activate();
                    });
                });
            }
            catch { }
        }
        catch (Exception ex)
        {
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Exception in OnLaunched: " + ex.Message + "\n" + ex.StackTrace + "\n");
        }
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

