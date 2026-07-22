using System;
using System.Collections.Specialized;
using Microsoft.UI.Dispatching;

namespace Deskdrop.WinUI
{
    public static class MessageBox
    {
        public static MessageBoxResult Show(string messageBoxText, string caption, MessageBoxButton button, MessageBoxImage icon)
        {
            return MessageBoxResult.OK;
        }
        public static MessageBoxResult Show(string messageBoxText)
        {
            return MessageBoxResult.OK;
        }
    }
    public enum MessageBoxResult { OK, Cancel, Yes, No }
    public enum MessageBoxButton { OK, OKCancel, YesNo, YesNoCancel }
    public enum MessageBoxImage { Error, Information, Warning, Question, None }
    
    public static class Clipboard
    {
        public static void SetText(string text) { }
        public static string GetText() { return ""; }
        public static void SetFileDropList(StringCollection filePaths) { }
        public static bool ContainsText() { return false; }
        public static bool ContainsFileDropList() { return false; }
        public static StringCollection GetFileDropList() { return new StringCollection(); }
        public static bool ContainsImage() { return false; }
        public static object? GetImage() { return null; }
    }

    public static class Dispatcher
    {
        public static void Invoke(Action action)
        {
            if (App.MainDispatcherQueue?.HasThreadAccess == true)
            {
                try { action(); } catch { }
            }
            else
            {
                var queue = App.MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
                queue?.TryEnqueue(() => { try { action(); } catch { } });
            }
        }
        public static void InvokeAsync(Action action)
        {
            var queue = App.MainDispatcherQueue ?? Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
            queue?.TryEnqueue(() => { try { action(); } catch { } });
        }
    }
}

