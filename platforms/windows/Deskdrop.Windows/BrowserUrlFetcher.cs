using System;
using System.Runtime.InteropServices;
using System.Windows.Automation;

namespace Deskdrop.Windows
{
    public static class BrowserUrlFetcher
    {
        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        public static async System.Threading.Tasks.Task<string?> GetActiveBrowserUrl()
        {
            try
            {
                var task = System.Threading.Tasks.Task.Run(() =>
                {
                    var foregroundWindow = GetForegroundWindow();
                    if (foregroundWindow == IntPtr.Zero) return (string?)null;

                    var windowElement = AutomationElement.FromHandle(foregroundWindow);
                    if (windowElement == null) return null;

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
                    return null;
                });
                
                var timeoutTask = System.Threading.Tasks.Task.Delay(500);
                if (await System.Threading.Tasks.Task.WhenAny(task, timeoutTask) == task)
                {
                    return await task;
                }
            }
            catch { /* Ignore automation exceptions */ }
            return null;
        }
    }
}
