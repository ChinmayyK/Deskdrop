// WindowsIpcServer.cs
// Full named-pipe IPC server for Windows.
// Replaces the stub in ipc.rs for the C# tray application.
//
// The Rust daemon writes JSON to \\.\pipe\deskdrop;
// the C# app (and deskdrop-cli on Windows) reads/writes the same pipe.

using System;
using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Deskdrop.Windows
{
    /// <summary>
    /// Named-pipe client that talks to the running Deskdrop daemon.
    /// Thread-safe: each request opens a fresh pipe connection.
    /// </summary>
    internal sealed class DaemonClient : IDisposable
    {
        private const string PipeName    = "deskdrop";
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

        public static JsonDocument? Ping()       => Send(new { cmd = "ping" });
        public static JsonDocument? Status()     => Send(new { cmd = "status" });
        public static JsonDocument? Peers()      => Send(new { cmd = "peers" });
        public static JsonDocument? RescanPeers() => Send(new { cmd = "rescan_peers" });

        public static JsonDocument? ConnectManual(string host, int? port = null)
        {
            object cmd = port.HasValue
                ? new { cmd = "connect_manual", host, port = port.Value }
                : (object)new { cmd = "connect_manual", host };
            return Send(cmd);
        }

        public static JsonDocument? PushClipboard(string? targetDeviceId = null)
        {
            object cmd = targetDeviceId != null
                ? new { cmd = "push_clipboard", target_device_id = targetDeviceId }
                : (object)new { cmd = "push_clipboard" };
            return Send(cmd);
        }

        public static JsonDocument? SaveSettings(object patch) =>
            Send(new { cmd = "save_settings" }); // caller passes full anon object

        // ── Private transport ─────────────────────────────────────────────────

        private static NamedPipeClientStream? OpenPipe(int timeoutMs)
        {
            var pipe = new NamedPipeClientStream(".", PipeName,
                PipeDirection.InOut, PipeOptions.None);
            try
            {
                pipe.Connect(timeoutMs);
                return pipe;
            }
            catch
            {
                pipe.Dispose();
                return null;
            }
        }

        private static string? ReadLineWithTimeout(Stream stream, int timeoutMs)
        {
            var sb   = new StringBuilder();
            var buf  = new byte[1];
            var dl   = DateTime.Now.AddMilliseconds(timeoutMs);
            while (DateTime.Now < dl)
            {
                if (stream.Read(buf, 0, 1) == 0) break;
                if (buf[0] == '\n') break;
                sb.Append((char)buf[0]);
            }
            return sb.Length > 0 ? sb.ToString() : null;
        }

        public void Dispose() { }
    }

    /// <summary>
    /// Polls the daemon on a background timer, surfacing connectivity and
    /// peer-count changes as events.  Uses adaptive interval: fast (1 s) when
    /// peers are connected for near-real-time UI; slow (5 s) when idle.
    /// </summary>
    internal sealed class DaemonPoller : IDisposable
    {

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
        // Fast poll when peers are connected; slow poll when idle.
        private const int FastMs = 1000;
        private const int SlowMs = 5000;

        private System.Threading.Timer? _timer;
        private bool _wasDaemonRunning;
        private int  _lastPeerCount        = -1;
        private bool _lastSyncState        = true;
        private int  _lastPendingClipboard = -1;

        public event Action<bool>? DaemonAvailabilityChanged;
        public event Action<int>?  PeerCountChanged;
        public event Action<bool>? SyncStateChanged;
        /// Fired when the number of unapplied incoming clipboard items changes.
        public event Action<int>?  PendingClipboardCountChanged;

        public DaemonPoller() => SchedulePoll(SlowMs);

        private void SchedulePoll(int delayMs)
        {
            _timer?.Dispose();
            _timer = new System.Threading.Timer(_ => Poll(), null, delayMs, Timeout.Infinite);
        }

        private void Poll()
        {
            bool running = DaemonClient.IsDaemonRunning();
            if (running != _wasDaemonRunning)
            {
                _wasDaemonRunning = running;
                DaemonAvailabilityChanged?.Invoke(running);
            }

            if (!running) { SchedulePoll(SlowMs); return; }

            var resp = DaemonClient.Status();
            if (resp == null) { SchedulePoll(SlowMs); return; }

            try
            {
                var root = resp.RootElement;
                if (root.TryGetProperty("data", out var data))
                {
                    int peerCount = data.TryGetProperty("peer_count", out var pc)
                        ? pc.GetInt32() : 0;
                    bool syncEnabled = !data.TryGetProperty("sync_enabled", out var se)
                        || se.GetBoolean();
                    int pending = data.TryGetProperty("pending_clipboard_count", out var pcc)
                        ? pcc.GetInt32() : 0;

                    if (peerCount != _lastPeerCount)
                    { _lastPeerCount = peerCount; PeerCountChanged?.Invoke(peerCount); }
                    if (syncEnabled != _lastSyncState)
                    { _lastSyncState = syncEnabled; SyncStateChanged?.Invoke(syncEnabled); }
                    if (pending != _lastPendingClipboard)
                    { _lastPendingClipboard = pending; PendingClipboardCountChanged?.Invoke(pending); }
                }
            }
            catch { }

            // Adaptive interval: fast when peers are present, slow otherwise.
            SchedulePoll(_lastPeerCount > 0 ? FastMs : SlowMs);
        }

        public void Dispose() { _timer?.Dispose(); }
    }
}
