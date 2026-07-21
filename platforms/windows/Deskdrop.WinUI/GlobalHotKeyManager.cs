using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace Deskdrop.WinUI
{
    public class GlobalHotKeyManager : IDisposable
    {
        public static GlobalHotKeyManager Shared { get; } = new GlobalHotKeyManager();

        public event Action? HotKeyPressed;

        private readonly HiddenWindow _window;
        private readonly Dictionary<int, Action> _callbacks = new();
        private int _currentId = 0;

        public GlobalHotKeyManager()
        {
            _window = new HiddenWindow();
            _window.HotKeyPressedInternal += OnHotKeyPressedInternal;
        }

        public void RegisterHotKey(bool ctrl, bool shift, bool alt, bool win, string key)
        {
            int id = ++_currentId;
            uint fsModifiers = 0;
            if (alt) fsModifiers |= 0x0001;
            if (ctrl) fsModifiers |= 0x0002;
            if (shift) fsModifiers |= 0x0004;
            if (win) fsModifiers |= 0x0008;

            uint vk = 0;
            if (!string.IsNullOrEmpty(key) && key.Length == 1)
            {
                vk = (uint)char.ToUpperInvariant(key[0]);
            }
            else if (key?.Equals("V", StringComparison.OrdinalIgnoreCase) == true) vk = 0x56;
            else if (key?.Equals("K", StringComparison.OrdinalIgnoreCase) == true) vk = 0x4B;

            if (vk != 0 && RegisterHotKey(_window.Handle, id, fsModifiers, vk))
            {
                _callbacks[id] = () => HotKeyPressed?.Invoke();
            }
        }

        public int Register(bool ctrl, bool shift, bool alt, bool win, uint vk, Action callback)
        {
            int id = ++_currentId;
            uint fsModifiers = 0;
            if (alt) fsModifiers |= 0x0001;
            if (ctrl) fsModifiers |= 0x0002;
            if (shift) fsModifiers |= 0x0004;
            if (win) fsModifiers |= 0x0008;

            if (RegisterHotKey(_window.Handle, id, fsModifiers, vk))
            {
                _callbacks[id] = callback;
                return id;
            }
            return -1;
        }

        public void UnregisterHotKey()
        {
            foreach (var id in _callbacks.Keys)
            {
                UnregisterHotKey(_window.Handle, id);
            }
            _callbacks.Clear();
        }

        private void OnHotKeyPressedInternal(int id)
        {
            if (_callbacks.TryGetValue(id, out var action))
            {
                action?.Invoke();
            }
        }

        public void Dispose()
        {
            UnregisterHotKey();
            _window.DestroyHandle();
        }

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool UnregisterHotKey(IntPtr hWnd, int id);

        private class HiddenWindow : NativeWindow
        {
            private const int WM_HOTKEY = 0x0312;
            public event Action<int>? HotKeyPressedInternal;

            public HiddenWindow()
            {
                CreateHandle(new CreateParams());
            }

            protected override void WndProc(ref Message m)
            {
                if (m.Msg == WM_HOTKEY)
                {
                    int id = m.WParam.ToInt32();
                    HotKeyPressedInternal?.Invoke(id);
                }
                base.WndProc(ref m);
            }
        }
    }
}





