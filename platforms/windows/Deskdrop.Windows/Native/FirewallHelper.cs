using System;

namespace Deskdrop.Windows
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

        /// Check for and add missing firewall rules. Runs silently — if the
        /// user doesn't have admin privileges the netsh calls will fail and
        /// we log a warning rather than crashing.
        public static void EnsureRules()
        {
            try
            {
                foreach (var (name, port, protocol) in RequiredRules)
                {
                    if (!RuleExists(name))
                    {
                        AddRule(name, port, protocol);
                    }
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"[Deskdrop] Firewall auto-config failed (non-fatal): {ex.Message}");
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

        private static void AddRule(string ruleName, int port, string protocol)
        {
            try
            {
                var psi = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = "netsh",
                    Arguments = $"advfirewall firewall add rule name=\"{ruleName}\" dir=in action=allow protocol={protocol} localport={port} profile=private,domain",
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true,
                };
                using var proc = System.Diagnostics.Process.Start(psi);
                if (proc != null)
                {
                    proc.WaitForExit(5000);
                    if (proc.ExitCode != 0)
                    {
                        System.Diagnostics.Debug.WriteLine(
                            $"[Deskdrop] Could not add firewall rule '{ruleName}' (may need admin privileges)");
                    }
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine(
                    $"[Deskdrop] Firewall rule '{ruleName}' add failed: {ex.Message}");
            }
        }
    }
}
