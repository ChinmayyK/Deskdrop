using Microsoft.UI.Dispatching;
using System;

namespace Deskdrop.WinUI
{
    internal static class FirewallHelper
    {
        private static readonly (string Name, int Port, string Protocol)[] RequiredRules = new[]
        {
            ("Deskdrop TCP",           47823, "TCP"),
            ("Deskdrop UDP Broadcast", 47824, "UDP"),
            ("Deskdrop UDP Multicast", 47825, "UDP"),
            ("Deskdrop mDNS",          5353,  "UDP"),
        };

        public static void EnsureRules()
        {
            try
            {
                bool needsElevation = false;
                foreach (var (name, port, protocol) in RequiredRules)
                {
                    if (!RuleExists(name))
                    {
                        needsElevation = true;
                        break;
                    }
                }

                if (needsElevation)
                {
                    System.Diagnostics.Debug.WriteLine("[Deskdrop] Missing firewall rules, attempting UAC elevation...");
                    AddRulesElevated();
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"[Deskdrop] Firewall auto-config failed (non-fatal): {ex.Message}");
            }
        }

        private static void AddRulesElevated()
        {
            try
            {
                string script = "";
                foreach (var (name, port, protocol) in RequiredRules)
                {
                    script += $"netsh advfirewall firewall add rule name=\\\"{name}\\\" dir=in action=allow protocol={protocol} localport={port} profile=any; ";
                }

                var psi = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = "powershell.exe",
                    Arguments = $"-NoProfile -ExecutionPolicy Bypass -Command \"{script}\"",
                    UseShellExecute = true,
                    Verb = "runas", // Triggers UAC prompt
                    WindowStyle = System.Diagnostics.ProcessWindowStyle.Hidden
                };

                using var proc = System.Diagnostics.Process.Start(psi);
                proc?.WaitForExit();
            }
            catch (System.ComponentModel.Win32Exception)
            {
                System.Diagnostics.Debug.WriteLine("[Deskdrop] User declined UAC prompt for Firewall rules.");
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"[Deskdrop] Elevated Firewall rule add failed: {ex.Message}");
            }
        }

        private static bool RuleExists(string ruleName)
        {
            try
            {
                var psi = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = "netsh",
                    Arguments = $"advfirewall firewall show rule name=\"{ruleName}\"",
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true,
                };
                using var proc = System.Diagnostics.Process.Start(psi);
                if (proc == null) return false;
                string output = proc.StandardOutput.ReadToEnd();
                proc.WaitForExit(3000);
                // netsh returns exit code 0 and prints rule details when it exists.
                return proc.ExitCode == 0 && output.Contains(ruleName);
            }
            catch { return false; }
        }
    }
}









