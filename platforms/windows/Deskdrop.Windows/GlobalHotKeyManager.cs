using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using System.Windows.Input;

namespace Deskdrop.Windows
{
    public class GlobalHotKeyManager : IDisposable
    {
        public static GlobalHotKeyManager Shared { get; } = new GlobalHotKeyManager();

        private readonly HiddenWindow _window;
        private readonly Dictionary<int, Action> _callbacks = new();
        private int _currentId = 0;

        private GlobalHotKeyManager()
        {
            _window = new HiddenWindow();
            _window.HotKeyPressed += OnHotKeyPressed;
        }

        public int Register(ModifierKeys modifiers, Key key, Action callback)
        {
            int id = ++_currentId;
            uint fsModifiers = 0;
            
            if ((modifiers & ModifierKeys.Alt) == ModifierKeys.Alt) fsModifiers |= 0x0001;
            if ((modifiers & ModifierKeys.Control) == ModifierKeys.Control) fsModifiers |= 0x0002;
            if ((modifiers & ModifierKeys.Shift) == ModifierKeys.Shift) fsModifiers |= 0x0004;
            if ((modifiers & ModifierKeys.Windows) == ModifierKeys.Windows) fsModifiers |= 0x0008;

            uint vk = (uint)KeyInterop.VirtualKeyFromKey(key);

            if (RegisterHotKey(_window.Handle, id, fsModifiers, vk))
            {
                _callbacks[id] = callback;
                return id;
            }
            return -1;
        }

        public void Unregister(int id)
        {
            if (_callbacks.ContainsKey(id))
            {
                UnregisterHotKey(_window.Handle, id);
                _callbacks.Remove(id);
            }
        }

        private void OnHotKeyPressed(int id)
        {
            if (_callbacks.TryGetValue(id, out var action))
            {
                action.Invoke();
            }
        }

        public void Dispose()
        {
            foreach (var id in _callbacks.Keys)
            {
                UnregisterHotKey(_window.Handle, id);
            }
            _callbacks.Clear();
            _window.DestroyHandle();
        }

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool UnregisterHotKey(IntPtr hWnd, int id);

        private class HiddenWindow : NativeWindow
        {
            private const int WM_HOTKEY = 0x0312;

            public event Action<int>? HotKeyPressed;

            public HiddenWindow()
            {
                CreateHandle(new CreateParams());
            }

            protected override void WndProc(ref Message m)
            {
                if (m.Msg == WM_HOTKEY)
                {
                    HotKeyPressed?.Invoke(m.WParam.ToInt32());
                }
                base.WndProc(ref m);
            }
        }
    }
}
