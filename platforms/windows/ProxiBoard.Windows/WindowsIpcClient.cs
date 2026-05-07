// WindowsIpcServer.cs
// Full named-pipe IPC server for Windows.
// Replaces the stub in ipc.rs for the C# tray application.
//
// The Rust daemon writes JSON to \\.\pipe\cliprelay;
// the C# app (and cliprelay-cli on Windows) reads/writes the same pipe.

using System;
using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace ClipRelay.Windows
{
    /// <summary>
    /// Named-pipe client that talks to the running ClipRelay daemon.
    /// Thread-safe: each request opens a fresh pipe connection.
    /// </summary>
    internal sealed class DaemonClient : IDisposable
    {
        private const string PipeName    = "cliprelay";
        private const int    TimeoutMs   = 1000;
        private bool _disposed;

        // ── Public API ────────────────────────────────────────────────────────

        /// <summary>
        /// Returns true if the daemon is currently reachable.
        /// </summary>
        public static bool IsDaemonRunning()
        {
            try
            {
                using var pipe = OpenPipe(TimeoutMs / 4);
                return pipe != null;
            }
            catch { return false; }
        }

        /// <summary>
        /// Send a JSON command and return the parsed response.
        /// Returns null if the daemon is not running.
        /// </summary>
        public static JsonDocument? Send(object request)
        {
            try
            {
                using var pipe = OpenPipe(TimeoutMs);
                if (pipe == null) return null;

                // Write request (newline-delimited JSON).
                var json = JsonSerializer.Serialize(request) + "\n";
                var bytes = Encoding.UTF8.GetBytes(json);
                pipe.Write(bytes, 0, bytes.Length);
                pipe.Flush();

                // Read response line.
                using var reader = new StreamReader(pipe, Encoding.UTF8, leaveOpen: true);
                var line = ReadLineWithTimeout(pipe, TimeoutMs);
                return line != null ? JsonDocument.Parse(line) : null;
            }
            catch { return null; }
        }

        // Async version for use in async contexts (tray event handlers).
        public static async Task<JsonDocument?> SendAsync(object request,
            CancellationToken ct = default)
        {
            return await Task.Run(() => Send(request), ct);
        }

        // ── Convenience commands ──────────────────────────────────────────────

        public static JsonDocument? Ping()   => Send(new { cmd = "ping" });
        public static JsonDocument? Status() => Send(new { cmd = "status" });
        public static JsonDocument? Peers()  => Send(new { cmd = "peers" });

        public static JsonDocument? PushText(string text) =>
            Send(new { cmd = "push_text", text });

        public static JsonDocument? SetSyncEnabled(bool enabled) =>
            Send(new { cmd = "set_sync_enabled", enabled });

        public static JsonDocument? HistoryClear() => Send(new { cmd = "history_clear" });

        public static JsonDocument? History(int last = 20) =>
            Send(new { cmd = "history", last });

        public static JsonDocument? RevokeTrustedDevice(string deviceId) =>
            Send(new { cmd = "revoke_trusted_device", device_id = deviceId });

        public static JsonDocument? Shutdown() => Send(new { cmd = "shutdown" });

        // ── Private helpers ───────────────────────────────────────────────────

        private static NamedPipeClientStream? OpenPipe(int timeoutMs)
        {
            var pipe = new NamedPipeClientStream(
                ".",            // server name (local)
                PipeName,
                PipeDirection.InOut,
                PipeOptions.Asynchronous);

            try
            {
                pipe.Connect(timeoutMs);
                pipe.ReadMode = PipeTransmissionMode.Byte;
                return pipe;
            }
            catch (TimeoutException)
            {
                pipe.Dispose();
                return null;
            }
            catch (IOException)
            {
                pipe.Dispose();
                return null;
            }
        }

        private static string? ReadLineWithTimeout(NamedPipeClientStream pipe, int timeoutMs)
        {
            var buf = new byte[65536];
            var sb  = new StringBuilder();
            var deadline = DateTime.UtcNow.AddMilliseconds(timeoutMs);

            while (DateTime.UtcNow < deadline)
            {
                if (!pipe.IsConnected) break;
                int n = 0;
                try { n = pipe.Read(buf, 0, buf.Length); }
                catch { break; }
                if (n == 0) break;

                var chunk = Encoding.UTF8.GetString(buf, 0, n);
                sb.Append(chunk);
                if (sb.ToString().Contains('\n'))
                    return sb.ToString().Split('\n')[0].Trim();
            }
            return null;
        }

        public void Dispose()
        {
            if (_disposed) return;
            _disposed = true;
        }
    }

    // ── Status poller ─────────────────────────────────────────────────────────

    /// <summary>
    /// Polls the daemon every N seconds and fires events on state changes.
    /// Used by the tray app to update the tooltip and menu items.
    /// </summary>
    internal sealed class DaemonPoller : IDisposable
    {
        private readonly Timer _timer;
        private bool _wasDaemonRunning;

        public event Action<bool>? DaemonAvailabilityChanged;
        public event Action<int>?  PeerCountChanged;
        public event Action<bool>? SyncStateChanged;

        private int  _lastPeerCount  = -1;
        private bool _lastSyncState  = true;

        public DaemonPoller(int intervalMs = 3000)
        {
            _timer = new Timer(_ => Poll(), null, 0, intervalMs);
        }

        private void Poll()
        {
            bool running = DaemonClient.IsDaemonRunning();
            if (running != _wasDaemonRunning)
            {
                _wasDaemonRunning = running;
                DaemonAvailabilityChanged?.Invoke(running);
            }

            if (!running) return;

            var resp = DaemonClient.Status();
            if (resp == null) return;

            try
            {
                var root = resp.RootElement;
                if (root.TryGetProperty("data", out var data))
                {
                    int peerCount = data.TryGetProperty("peer_count", out var pc)
                        ? pc.GetInt32() : 0;

                    bool syncEnabled = !data.TryGetProperty("sync_enabled", out var se)
                        || se.GetBoolean();

                    if (peerCount != _lastPeerCount)
                    {
                        _lastPeerCount = peerCount;
                        PeerCountChanged?.Invoke(peerCount);
                    }

                    if (syncEnabled != _lastSyncState)
                    {
                        _lastSyncState = syncEnabled;
                        SyncStateChanged?.Invoke(syncEnabled);
                    }
                }
            }
            catch { /* JSON shape mismatch — ignore */ }
        }

        public void Dispose() => _timer.Dispose();
    }
}
