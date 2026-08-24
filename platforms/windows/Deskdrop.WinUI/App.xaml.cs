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
    private static Deskdrop.WinUI.Services.GlobalDragMonitor? _dragMonitor;
    private static Deskdrop.WinUI.Services.ScreenshotObserver? _screenshotObserver;

    // Lets Settings flip screenshot auto-sync on/off live, without an app
    // restart, while still persisting the choice for next launch.
    public static void SetScreenshotSyncEnabled(bool enabled)
    {
        try
        {
            Deskdrop.WinUI.Services.LocalSettingsStore.SetBool("ScreenshotSyncEnabled", enabled);
            if (enabled && _screenshotObserver == null && Clipboard != null)
            {
                _screenshotObserver = new Deskdrop.WinUI.Services.ScreenshotObserver(Clipboard);
            }
            else if (!enabled && _screenshotObserver != null)
            {
                _screenshotObserver.Dispose();
                _screenshotObserver = null;
            }
        }
        catch (Exception ex) { App.HandleError(ex); }
    }

    public static bool ScreenshotSyncEnabled =>
        Deskdrop.WinUI.Services.LocalSettingsStore.GetBool("ScreenshotSyncEnabled");

    [System.Runtime.InteropServices.DllImport("shell32.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
    private static extern int SetCurrentProcessExplicitAppUserModelID(string AppID);

    public App()
    {
        MainDispatcherQueue = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        var dir = System.IO.Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
        System.IO.Directory.CreateDirectory(dir);

        System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] App constructor started\n");

        // Unpackaged Win32/WinUI3 apps need to explicitly claim an
        // AppUserModelID before the notification platform will reliably
        // deliver toasts for that ID (NotificationHelper.AppUserModelID) -
        // without this, ToastNotifier.Show() can silently no-op instead of
        // throwing, which is very easy to mistake for "notifications are
        // broken" when it's really just missing identity registration.
        try { SetCurrentProcessExplicitAppUserModelID(NotificationHelper.AppUserModelID); }
        catch (Exception ex) { App.HandleError(ex); }

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
            // Single instance via AppInstance key redirection (not a raw
            // Mutex): a Mutex can only tell a second launch "someone's
            // already running" and exit - it has no way to hand that
            // second launch's activation args (e.g. the file path from
            // Explorer's "Send via Deskdrop") to the first instance. This
            // redirects the whole AppActivationArguments to the existing
            // instance's OnAppActivated, so ProcessActivationArgs actually
            // sees it instead of the file silently getting dropped whenever
            // Deskdrop was already running in the tray (the common case).
            _keyInstance = Microsoft.Windows.AppLifecycle.AppInstance.FindOrRegisterForKey("Deskdrop_Main_Instance");
            var activatedArgs = Microsoft.Windows.AppLifecycle.AppInstance.GetCurrent().GetActivatedEventArgs();

            if (!_keyInstance.IsCurrent)
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Secondary instance detected. Redirecting activation to existing instance.\n");
                _keyInstance.RedirectActivationToAsync(activatedArgs).AsTask().Wait();
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

            _keyInstance.Activated += OnAppActivated;

            Deskdrop.WinUI.Native.ContextMenuIntegration.RegisterContextMenu();
            Deskdrop.WinUI.Native.ContextMenuIntegration.RegisterUriProtocol();
            NotificationHelper.EnsureRegistered();

            // Process initial launch arguments
            ProcessActivationArgs(activatedArgs);

            // Best-effort: add the firewall rules Deskdrop needs for LAN
            // discovery/transfer if they're missing. May trigger a UAC
            // prompt on first run - never block startup on it.
            System.Threading.Tasks.Task.Run(() =>
            {
                try { Deskdrop.WinUI.FirewallHelper.EnsureRules(); }
                catch (Exception ex) { App.HandleError(ex); }
            });

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

            try
            {
                _dragMonitor = new Deskdrop.WinUI.Services.GlobalDragMonitor(Clipboard);
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

            try
            {
                if (ScreenshotSyncEnabled)
                {
                    _screenshotObserver = new Deskdrop.WinUI.Services.ScreenshotObserver(Clipboard);
                }
            }
            catch (Exception ex) { App.HandleError(ex); }

            try
            {
                // Ctrl+Shift+V: Quick Access (clipboard timeline + device list)
                GlobalHotKeyManager.Shared.Register(true, true, false, false, 0x56, () => {
                    var queue = MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
                    queue?.TryEnqueue(() => { try { new QuickAccessWindow().Activate(); } catch (Exception ex) { App.HandleError(ex); } });
                });
                // Ctrl+K: bring the main Dashboard window to the front
                GlobalHotKeyManager.Shared.Register(true, false, false, false, 0x4B, () => {
                    ShowMainWindowCommand?.Execute(null);
                });
            }
            catch (Exception ex) { App.HandleError(ex); }
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

    // Shared dispatch for deskdrop://accept/{id} and deskdrop://reject/{id}
    // (still used as plain argument strings even though the notification
    // API that carries them changed) - called from both a cold launch
    // (ProcessActivationArgs, AppNotification/Protocol kinds) and a click
    // while already running (NotificationHelper.OnNotificationInvoked).
    public static void HandleDeskdropUri(string uriString)
    {
        if (!Uri.TryCreate(uriString, UriKind.Absolute, out var uri) || uri.Scheme != "deskdrop") return;
        if (uri.Host != "accept" && uri.Host != "reject") return;

        var transferId = uri.AbsolutePath.Trim('/');
        if (string.IsNullOrEmpty(transferId)) return;

        try
        {
            if (uri.Host == "accept") DaemonClient.AcceptFileTransfer(transferId);
            else DaemonClient.RejectFileTransfer(transferId, "user_declined");
        }
        catch (Exception ex) { App.HandleError(ex); }
    }

    private void ProcessActivationArgs(Microsoft.Windows.AppLifecycle.AppActivationArguments activatedArgs)
    {
        var dir = System.IO.Path.Combine(System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop"));
        if (activatedArgs.Kind == Microsoft.Windows.AppLifecycle.ExtendedActivationKind.Protocol)
        {
            var protocolArgs = activatedArgs.Data as Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs;
            var uri = protocolArgs?.Uri;
            if (uri != null)
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Activated via protocol: " + uri.ToString() + "\n");
                HandleDeskdropUri(uri.ToString());
            }
        }
        else if (activatedArgs.Kind == Microsoft.Windows.AppLifecycle.ExtendedActivationKind.AppNotification)
        {
            // Cold-launch equivalent of NotificationHelper.OnNotificationInvoked:
            // the app wasn't running when an Accept/Reject notification
            // button was clicked, so Windows launched it fresh with this
            // activation kind instead of raising that in-process event.
            if (activatedArgs.Data is Microsoft.Windows.AppNotifications.AppNotificationActivatedEventArgs notifArgs
                && notifArgs.Arguments.TryGetValue("action", out var action) && !string.IsNullOrEmpty(action))
            {
                System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "winui_trace.txt"), "[" + DateTime.Now.ToString("u") + "] Activated via AppNotification: " + action + "\n");
                HandleDeskdropUri(action);
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

    private static Microsoft.Windows.AppLifecycle.AppInstance? _keyInstance;

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

