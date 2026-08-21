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
    public static Microsoft.UI.Dispatching.DispatcherQueue? MainDispatcherQueue { get; private set; }
    public static bool IsShuttingDown { get; private set; } = false;
    
    // Commands
    public System.Windows.Input.ICommand ShowMainWindowCommand { get; }
    public System.Windows.Input.ICommand ExitApplicationCommand { get; }

    private static IntPtr _engineHandle = IntPtr.Zero;
    public static IntPtr EngineHandle => _engineHandle;
    public static Deskdrop.WinUI.Services.ClipboardManager? Clipboard { get; private set; }
    public static System.Threading.Tasks.TaskCompletionSource<Deskdrop.WinUI.Services.ClipboardManager> ClipboardReady { get; } = new();

    public App()
    {
        MainDispatcherQueue = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        var dir = System.IO.Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
        System.IO.Directory.CreateDirectory(dir);
        
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App constructor started\n");

        this.UnhandledException += (s, e) =>
        {
            e.Handled = true;
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] WinUI UnhandledException: {e.Exception?.Message}\n{e.Exception?.StackTrace}\n"); } catch (Exception ex) { App.HandleError(ex); }
        };
        AppDomain.CurrentDomain.UnhandledException += (s, e) =>
        {
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] AppDomain UnhandledException: {(e.ExceptionObject as Exception)?.Message}\n"); } catch (Exception ex) { App.HandleError(ex); }
        };
        System.Threading.Tasks.TaskScheduler.UnobservedTaskException += (s, e) =>
        {
            e.SetObserved();
            try { System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] TaskScheduler UnobservedTaskException: {e.Exception?.Message}\n"); } catch (Exception ex) { App.HandleError(ex); }
        };

        InitializeComponent();
        
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App InitializeComponent finished\n");

        ShowMainWindowCommand = new RelayCommand(() =>
        {
            var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            queue?.TryEnqueue(() =>
            {
                try
                {
                    if (MainWindow == null || DashboardWindow.Current == null)
                    {
                        MainWindow = new DashboardWindow();
                        _window = MainWindow;
                        MainWindow.Activate();
                    }
                    else
                    {
                        try
                        {
                            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(MainWindow);
                            ShowWindow(hwnd, 9 /* SW_RESTORE */);
                            SetForegroundWindow(hwnd);
                            MainWindow.Activate();
                        }
                        catch
                        {
                            MainWindow = new DashboardWindow();
                            _window = MainWindow;
                            MainWindow.Activate();
                        }
                    }
                }
                catch (Exception ex)
                {
                    App.HandleError(ex);
                }
            });
        });
        
        ExitApplicationCommand = new RelayCommand(() =>
        {
            IsShuttingDown = true;
            var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            queue?.TryEnqueue(() =>
            {
                try { Deskdrop.WinUI.Services.TrayService.Current?.Dispose(); } catch (Exception ex) { App.HandleError(ex); }
                if (_engineHandle != IntPtr.Zero)
                {
                    try { Deskdrop.WinUI.NativeCore.deskdrop_stop(_engineHandle); _engineHandle = IntPtr.Zero; } catch (Exception ex) { App.HandleError(ex); }
                }
                try { GlobalHotKeyManager.Shared.Dispose(); } catch (Exception ex) { App.HandleError(ex); }
                try { Application.Current.Exit(); } catch (Exception ex) { App.HandleError(ex); }
                Environment.Exit(0);
            });
        });
    }

    protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
    {
        var dir = System.IO.Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
        System.IO.Directory.CreateDirectory(dir);
        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] OnLaunched started\n");

        try
        {
            // Clean Single Instance using Mutex
            _singleInstanceMutex = new Mutex(true, "Local\\Deskdrop_WinUI_App_Unique_Key", out bool createdNew);
            if (!createdNew)
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Secondary instance detected. Bringing existing window to front and exiting.\n");
                var existingHwnd = FindWindowW(null, "Deskdrop");
                if (existingHwnd == IntPtr.Zero) existingHwnd = FindWindowW(null, "DeskDrop Dashboard");
                if (existingHwnd != IntPtr.Zero)
                {
                    ShowWindow(existingHwnd, 9 /* SW_RESTORE */);
                    SetForegroundWindow(existingHwnd);
                }
                Environment.Exit(0);
                return;
            }

            Deskdrop.WinUI.Native.ContextMenuIntegration.RegisterContextMenu();

            // Process initial launch arguments
            var activatedArgs = Microsoft.Windows.AppLifecycle.AppInstance.GetCurrent().GetActivatedEventArgs();
            ProcessActivationArgs(activatedArgs);

            // Initialize the in-process native core FFI
            System.Threading.Tasks.Task.Run(() =>
            {
                try
                {
                    _engineHandle = Deskdrop.WinUI.NativeCore.deskdrop_start(System.Environment.MachineName, 0);
                    System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Native engine started gracefully. Handle: " + _engineHandle + "\n");
                }
                catch (Exception ex)
                {
                    System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Native engine start failed: " + ex.Message + "\n");
                }
            });

            Clipboard = new Deskdrop.WinUI.Services.ClipboardManager();
            ClipboardReady.TrySetResult(Clipboard);

            try
            {
                _ = new Deskdrop.WinUI.Services.TrayService();
            }
            catch (Exception ex) { App.HandleError(ex); }

            MainDispatcherQueue ??= Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();

            try
            {
                MainWindow = new DashboardWindow();
                _window = MainWindow;
                _window.Activate();
                
                var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(MainWindow);
                ShowWindow(hwnd, 5 /* SW_SHOW */);
                SetForegroundWindow(hwnd);
                
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] MainWindow created, activated, and displayed successfully\n");
            }
            catch (Exception ex)
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] MainWindow crash: " + ex.ToString() + "\n");
                if (ex.InnerException != null) System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "Inner: " + ex.InnerException.ToString() + "\n");
            }

            /*
            try
            {
                GlobalHotKeyManager.Shared.Register(true, true, false, false, 0x56, () => {
                    var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
                    queue?.TryEnqueue(() => { try { new QuickAccessWindow().Activate(); } catch (Exception ex) { App.HandleError(ex); } });
                });
                GlobalHotKeyManager.Shared.Register(true, false, false, false, 0x4B, () => {
                    ShowMainWindowCommand?.Execute(null);
                });
            }
            catch (Exception ex) { App.HandleError(ex); }
            */
        }
        catch (Exception ex)
        {
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Exception in OnLaunched: " + ex.Message + "\n" + ex.StackTrace + "\n");
        }
    }
    private void OnAppActivated(object? sender, Microsoft.Windows.AppLifecycle.AppActivationArguments e)
    {
        ProcessActivationArgs(e);
    }

    private void ProcessActivationArgs(Microsoft.Windows.AppLifecycle.AppActivationArguments activatedArgs)
    {
        var dir = System.IO.Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
        if (activatedArgs.Kind == Microsoft.Windows.AppLifecycle.ExtendedActivationKind.Protocol)
        {
            var protocolArgs = activatedArgs.Data as Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs;
            var uri = protocolArgs?.Uri;
            if (uri != null && uri.Scheme == "deskdrop")
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Activated via protocol: " + uri.ToString() + "\n");
            }
        }
        else if (activatedArgs.Kind == Microsoft.Windows.AppLifecycle.ExtendedActivationKind.CommandLineLaunch)
        {
            var cmdLineArgs = activatedArgs.Data as Windows.ApplicationModel.Activation.ICommandLineActivatedEventArgs;
            if (cmdLineArgs != null)
            {
                string argsStr = cmdLineArgs.Operation.Arguments;
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Activated via CommandLine: " + argsStr + "\n");
                
                var matches = System.Text.RegularExpressions.Regex.Matches(argsStr, @"[\""].+?[\""]|[^ ]+");
                if (matches.Count >= 2)
                {
                    string path = matches[matches.Count - 1].Value.Trim('"');
                    if (System.IO.File.Exists(path) || System.IO.Directory.Exists(path))
                    {
                        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Context Menu Trigger: Sending " + path + "\n");
                        
                        System.Threading.Tasks.Task.Run(async () =>
                        {
                            var clipboard = await ClipboardReady.Task;
                            try
                            {
                                clipboard.PushFile(path);
                            }
                            catch (Exception e)
                            {
                                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] Error pushing context menu file: {e.Message}\n");
                            }
                        });
                    }
                }
            }
        }
    }

    private static System.Threading.Mutex? _singleInstanceMutex;

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    private static extern IntPtr FindWindowW(string? lpClassName, string? lpWindowName);

    public static void HandleError(Exception ex, [System.Runtime.CompilerServices.CallerMemberName] string callerName = "")
    {
        try
        {
            var dir = System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.Now:u}] Swallowed Exception in {callerName}: {ex.Message}\n{ex.StackTrace}\n");
        }
        catch { } // Failsafe
    }
}

public class RelayCommand : System.Windows.Input.ICommand
{
    private readonly Action _execute;
    public RelayCommand(Action execute) => _execute = execute;
    public event EventHandler? CanExecuteChanged { add { } remove { } }
    public bool CanExecute(object? parameter) => true;
    public void Execute(object? parameter) => _execute();
}

