using System;
namespace Deskdrop.WinUI
{
    public class GlobalHotKeyManager : IDisposable
    {
        public event Action HotKeyPressed;
        public void RegisterHotKey(bool ctrl, bool shift, bool alt, bool win, string key) { }
        public void UnregisterHotKey() { }
        public void Dispose() { }
    }
}





