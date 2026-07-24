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
    
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    private const int SW_RESTORE = 9;

    // Tray Icon Properties
    public H.NotifyIcon.TaskbarIcon? TrayIcon { get; private set; }
    public static Microsoft.UI.Dispatching.DispatcherQueue? MainDispatcherQueue { get; private set; }
    public static bool IsShuttingDown { get; private set; } = false;
    
    // Commands
    public System.Windows.Input.ICommand ShowMainWindowCommand { get; }
    public System.Windows.Input.ICommand ExitApplicationCommand { get; }

    private static System.Threading.Mutex? _singleInstanceMutex;
    private static IntPtr _engineHandle = IntPtr.Zero;
    public static IntPtr EngineHandle => _engineHandle;
    public static Deskdrop.WinUI.Services.ClipboardManager? Clipboard { get; private set; }

    public App()
    {
        MainDispatcherQueue = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        var dir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
        
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App constructor started\n");

        this.UnhandledException += (s, e) =>
        {
            e.Handled = true;
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] WinUI UnhandledException: {e.Exception?.Message}\n{e.Exception?.StackTrace}\n"); } catch { }
        };
        AppDomain.CurrentDomain.UnhandledException += (s, e) =>
        {
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] AppDomain UnhandledException: {(e.ExceptionObject as Exception)?.Message}\n"); } catch { }
        };
        System.Threading.Tasks.TaskScheduler.UnobservedTaskException += (s, e) =>
        {
            e.SetObserved();
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] TaskScheduler UnobservedTaskException: {e.Exception?.Message}\n"); } catch { }
        };

        InitializeComponent();
        
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App InitializeComponent finished\n");

        ShowMainWindowCommand = new RelayCommand(() =>
        {
            var dir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory));
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] ShowMainWindowCommand invoked\n"); } catch { }

            var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            queue?.TryEnqueue(() =>
            {
                try
                {
                    if (MainWindow == null || DashboardWindow.Current == null)
                    {
                        MainWindow = new DashboardWindow();
                    }
                    else
                    {
                        try
                        {
                            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(MainWindow);
                            var windowId = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
                            var appWindow = Microsoft.UI.Windowing.AppWindow.GetFromWindowId(windowId);
                            appWindow.Show();
                            ShowWindow(hwnd, SW_RESTORE);
                            SetForegroundWindow(hwnd);
                        }
                        catch
                        {
                            MainWindow = new DashboardWindow();
                        }
                    }
                    _window = MainWindow;
                    MainWindow.Activate();
                    try
                    {
                        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(MainWindow);
                        ShowWindow(hwnd, SW_RESTORE);
                        SetForegroundWindow(hwnd);
                    }
                    catch { }
                }
                catch (Exception ex)
                {
                    try
                    {
                        MainWindow = new DashboardWindow();
                        _window = MainWindow;
                        MainWindow.Activate();
                    }
                    catch { }
                }
            });
        });
        
        ExitApplicationCommand = new RelayCommand(() =>
        {
            IsShuttingDown = true;
            var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            queue?.TryEnqueue(() =>
            {
                try { TrayIcon?.Dispose(); } catch { }
                if (_engineHandle != IntPtr.Zero)
                {
                    try { NativeCore.deskdrop_stop(_engineHandle); _engineHandle = IntPtr.Zero; } catch { }
                }
                try { GlobalHotKeyManager.Shared.Dispose(); } catch { }
                try { Application.Current.Exit(); } catch { }
                Environment.Exit(0);
            });
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

            MainDispatcherQueue ??= Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();

            // Initialize the Tray Icon entirely in C# to avoid x:Bind issues in App.xaml
            TrayIcon = new H.NotifyIcon.TaskbarIcon
            {
                ToolTipText = "Deskdrop",
                IconSource = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri("ms-appx:///Assets/AppIcon.ico")),
                LeftClickCommand = ShowMainWindowCommand,
                DoubleClickCommand = ShowMainWindowCommand
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
                    var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
                    queue?.TryEnqueue(() => { try { new QuickAccessWindow().Activate(); } catch { } });
                });
                GlobalHotKeyManager.Shared.Register(true, false, false, false, 0x4B, () => {
                    ShowMainWindowCommand?.Execute(null);
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

