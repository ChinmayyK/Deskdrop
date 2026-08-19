using System;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;

namespace Deskdrop.WinUI.Services
{
    public class TrayService : IDisposable
    {
        private const int WM_USER = 0x0400;
        private const int WM_TRAYICON = WM_USER + 101;
        private const int WM_LBUTTONUP = 0x0202;
        private const int WM_LBUTTONDBLCLK = 0x0203;
        private const int WM_RBUTTONUP = 0x0205;

        private const int NIM_ADD = 0x0000;
        private const int NIM_MODIFY = 0x0001;
        private const int NIM_DELETE = 0x0002;

        private const int NIF_MESSAGE = 0x0001;
        private const int NIF_ICON = 0x0002;
        private const int NIF_TIP = 0x0004;

        private const uint MF_STRING = 0x00000000;
        private const uint MF_SEPARATOR = 0x00000800;
        private const uint MF_DEFAULT = 0x00001000;
        private const uint TPM_RIGHTBUTTON = 0x0002;
        private const uint TPM_RETURNCMD = 0x0100;

        private const int IDM_OPEN = 1001;
        private const int IDM_SETTINGS = 1002;
        private const int IDM_RESCAN = 1003;
        private const int IDM_QUIT = 1004;

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct NOTIFYICONDATA
        {
            public int cbSize;
            public IntPtr hWnd;
            public int uID;
            public int uFlags;
            public int uCallbackMessage;
            public IntPtr hIcon;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)]
            public string szTip;
            public int dwState;
            public int dwStateMask;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)]
            public string szInfo;
            public int uTimeoutOrVersion;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)]
            public string szInfoTitle;
            public int dwInfoFlags;
            public Guid guidItem;
            public IntPtr hBalloonIcon;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct POINT
        {
            public int x;
            public int y;
        }

        private delegate IntPtr SubclassProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, UIntPtr dwRefData);

        [DllImport("comctl32.dll", SetLastError = true)]
        private static extern bool SetWindowSubclass(IntPtr hWnd, SubclassProc pfnSubclass, UIntPtr uIdSubclass, UIntPtr dwRefData);

        [DllImport("comctl32.dll", SetLastError = true)]
        private static extern bool RemoveWindowSubclass(IntPtr hWnd, SubclassProc pfnSubclass, UIntPtr uIdSubclass);

        [DllImport("comctl32.dll")]
        private static extern IntPtr DefSubclassProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam);

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern bool Shell_NotifyIconW(int dwMessage, ref NOTIFYICONDATA lpData);

        [DllImport("user32.dll")]
        private static extern IntPtr CreatePopupMenu();

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern bool AppendMenuW(IntPtr hMenu, uint uFlags, uint uIDNewItem, string lpNewItem);

        [DllImport("user32.dll")]
        private static extern uint TrackPopupMenuEx(IntPtr hMenu, uint uFlags, int x, int y, IntPtr hWnd, IntPtr lpTPMPARAMS);

        [DllImport("user32.dll")]
        private static extern bool DestroyMenu(IntPtr hMenu);

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool GetCursorPos(out POINT lpPoint);

        [DllImport("user32.dll")]
        private static extern uint RegisterWindowMessageW(string lpString);

        [DllImport("user32.dll")]
        private static extern bool DestroyIcon(IntPtr hIcon);

        private IntPtr _hWnd = IntPtr.Zero;
        private IntPtr _hIcon = IntPtr.Zero;
        private SubclassProc? _subclassProc;
        private uint _taskbarCreatedMsg = 0;
        private bool _isCreated = false;

        public static TrayService? Current { get; private set; }

        public TrayService(IntPtr targetHwnd)
        {
            Current = this;
            _hWnd = targetHwnd;
            try
            {
                Initialize();
            }
            catch (Exception ex)
            {
                Log("Init error: " + ex.ToString());
            }
        }

        private void Initialize()
        {
            if (_hWnd == IntPtr.Zero)
            {
                Log("Initialize: targetHwnd is zero!");
                return;
            }

            _subclassProc = CustomSubclassProc;
            SetWindowSubclass(_hWnd, _subclassProc, (UIntPtr)101, UIntPtr.Zero);
            _taskbarCreatedMsg = RegisterWindowMessageW("TaskbarCreated");

            LoadIconHandle();
            AddTrayIcon();
        }

        private void LoadIconHandle()
        {
            try
            {
                var baseDir = AppContext.BaseDirectory;
                var trayIcoPath = Path.Combine(baseDir, "Assets", "TrayIcon.ico");
                if (!File.Exists(trayIcoPath))
                {
                    trayIcoPath = Path.Combine(baseDir, "Assets", "AppIcon.ico");
                }

                if (File.Exists(trayIcoPath))
                {
                    var icon = new System.Drawing.Icon(trayIcoPath, 16, 16);
                    _hIcon = icon.Handle;
                }
                else
                {
                    _hIcon = System.Drawing.SystemIcons.Application.Handle;
                }
            }
            catch (Exception ex)
            {
                Log("LoadIconHandle error: " + ex.Message);
                _hIcon = System.Drawing.SystemIcons.Application.Handle;
            }
        }

        private void AddTrayIcon()
        {
            if (_hWnd == IntPtr.Zero) return;

            System.Threading.Tasks.Task.Run(async () =>
            {
                for (int attempt = 1; attempt <= 10; attempt++)
                {
                    var nid = new NOTIFYICONDATA
                    {
                        cbSize = Marshal.SizeOf<NOTIFYICONDATA>(),
                        hWnd = _hWnd,
                        uID = 100,
                        uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP,
                        uCallbackMessage = WM_TRAYICON,
                        hIcon = _hIcon,
                        szTip = "Deskdrop - Ready"
                    };

                    _isCreated = Shell_NotifyIconW(NIM_ADD, ref nid);
                    var err = Marshal.GetLastWin32Error();
                    Log($"AddTrayIcon attempt {attempt}: created={_isCreated}, LastError={err}");

                    if (_isCreated)
                    {
                        break;
                    }

                    await System.Threading.Tasks.Task.Delay(1000);
                }
            });
        }

        public void UpdateTooltip(string text)
        {
            if (!_isCreated || _hWnd == IntPtr.Zero) return;
            var nid = new NOTIFYICONDATA
            {
                cbSize = Marshal.SizeOf<NOTIFYICONDATA>(),
                hWnd = _hWnd,
                uID = 1,
                uFlags = NIF_TIP,
                szTip = text.Length > 120 ? text.Substring(0, 120) : text
            };
            Shell_NotifyIconW(NIM_MODIFY, ref nid);
        }

        private IntPtr CustomSubclassProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, UIntPtr dwRefData)
        {
            if (msg == WM_TRAYICON)
            {
                var eventType = (uint)lParam.ToInt64();
                if (eventType == WM_LBUTTONUP || eventType == WM_LBUTTONDBLCLK)
                {
                    App.MainDispatcherQueue?.TryEnqueue(() =>
                    {
                        ((App)Application.Current).ShowMainWindowCommand?.Execute(null);
                    });
                }
                else if (eventType == WM_RBUTTONUP)
                {
                    ShowMenu();
                }
                return IntPtr.Zero;
            }
            else if (_taskbarCreatedMsg != 0 && msg == _taskbarCreatedMsg)
            {
                AddTrayIcon();
                return IntPtr.Zero;
            }

            return DefSubclassProc(hWnd, msg, wParam, lParam);
        }

        private void ShowMenu()
        {
            var hMenu = CreatePopupMenu();
            if (hMenu == IntPtr.Zero) return;

            AppendMenuW(hMenu, MF_STRING | MF_DEFAULT, IDM_OPEN, "Open Deskdrop");
            AppendMenuW(hMenu, MF_STRING, IDM_SETTINGS, "Settings...");
            AppendMenuW(hMenu, MF_STRING, IDM_RESCAN, "Rescan Network");
            AppendMenuW(hMenu, MF_SEPARATOR, 0, "");
            AppendMenuW(hMenu, MF_STRING, IDM_QUIT, "Quit Deskdrop");

            GetCursorPos(out var pt);
            SetForegroundWindow(_hWnd);

            var cmd = TrackPopupMenuEx(hMenu, TPM_RIGHTBUTTON | TPM_RETURNCMD, pt.x, pt.y, _hWnd, IntPtr.Zero);
            DestroyMenu(hMenu);

            if (cmd != 0)
            {
                App.MainDispatcherQueue?.TryEnqueue(() =>
                {
                    switch (cmd)
                    {
                        case IDM_OPEN:
                            ((App)Application.Current).ShowMainWindowCommand?.Execute(null);
                            break;
                        case IDM_SETTINGS:
                            ((App)Application.Current).ShowMainWindowCommand?.Execute(null);
                            DashboardWindow.Current?.NavigateTo("Settings");
                            break;
                        case IDM_RESCAN:
                            DaemonClient.RescanPeers();
                            break;
                        case IDM_QUIT:
                            ((App)Application.Current).ExitApplicationCommand?.Execute(null);
                            break;
                    }
                });
            }
        }

        private static void Log(string msg)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Deskdrop");
                File.AppendAllText(Path.Combine(dir, "winui_trace.txt"), $"[{DateTime.UtcNow:u}] [TrayService] {msg}\n");
            }
            catch { }
        }

        public void Dispose()
        {
            if (_isCreated && _hWnd != IntPtr.Zero)
            {
                var nid = new NOTIFYICONDATA
                {
                    cbSize = Marshal.SizeOf<NOTIFYICONDATA>(),
                    hWnd = _hWnd,
                    uID = 100
                };
                Shell_NotifyIconW(NIM_DELETE, ref nid);
                _isCreated = false;
            }

            if (_subclassProc != null && _hWnd != IntPtr.Zero)
            {
                RemoveWindowSubclass(_hWnd, _subclassProc, (UIntPtr)101);
                _subclassProc = null;
            }

            if (_hIcon != IntPtr.Zero)
            {
                DestroyIcon(_hIcon);
                _hIcon = IntPtr.Zero;
            }
        }
    }
}
