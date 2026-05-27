using System;
using System.Runtime.InteropServices;
using System.Windows.Automation;

namespace Deskdrop.Windows
{
    public static class BrowserUrlFetcher
    {
        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        public static string? GetActiveBrowserUrl()
        {
            try
            {
                var foregroundWindow = GetForegroundWindow();
                if (foregroundWindow == IntPtr.Zero) return null;

                var windowElement = AutomationElement.FromHandle(foregroundWindow);
                if (windowElement == null) return null;

                // Look for the address bar (Edit control)
                var editControlCondition = new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Edit);
                var element = windowElement.FindFirst(TreeScope.Descendants, editControlCondition);

                if (element != null && element.TryGetCurrentPattern(ValuePattern.Pattern, out object patternObj))
                {
                    var valuePattern = (ValuePattern)patternObj;
                    string val = valuePattern.Current.Value;
                    
                    if (!string.IsNullOrWhiteSpace(val))
                    {
                        if (!val.StartsWith("http") && !val.Contains("://"))
                        {
                            val = "https://" + val;
                        }
                        if (Uri.IsWellFormedUriString(val, UriKind.Absolute))
                        {
                            return val;
                        }
                    }
                }
            }
            catch { /* Ignore automation exceptions */ }
            return null;
        }
    }
}
