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
        private static string PipeName => "deskdrop";
        private const int    TimeoutMs   = 1000;


        public static JsonDocument? SendFilePath(string path, string name, string mime, string? targetDevice = null)
        {
            if (targetDevice == null)
            {
                return Send(new { cmd = "send_file_path", path = path, name = name, mime = mime });
            }
            return Send(new { cmd = "send_file_path", path = path, name = name, mime = mime, target_device = targetDevice });
        }

        // ── Security ────────────────────────────────────────────────────────────

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
                if (line != null)
                {
                    var doc = JsonDocument.Parse(line);
                    if (doc.RootElement.TryGetProperty("status", out var st) && st.GetString() == "error")
                    {
                        var msg = doc.RootElement.TryGetProperty("message", out var err) ? err.GetString() : "Unknown IPC error";
                        throw new InvalidOperationException($"IPC returned error: {msg}");
                    }
                    return doc;
                }
                return null;
            }
            catch (InvalidOperationException) { throw; }
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
        public static JsonDocument? ConnectManual(string host, int? port = null)
        {
            object cmd = port.HasValue
                ? new { cmd = "connect_peer", ip = host, port = port.Value }
                : (object)new { cmd = "connect_peer", ip = host, port = 47823 };
            return Send(cmd);
        }

        public static JsonDocument? PatchSettings(object patch)
        {
            return Send(new { cmd = "patch_settings", patch = JsonSerializer.Serialize(patch) });
        }

        public static JsonDocument? LatestCameraFrame(string peerId) => Send(new { cmd = "latest_camera_frame", target_device = peerId });

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

        public static JsonDocument? PushText(string text) =>
            Send(new { cmd = "push_text", text });

        public static JsonDocument? SetSyncEnabled(bool enabled) =>
            Send(new { cmd = "set_sync_enabled", enabled });

        public static JsonDocument? HistoryClear() => Send(new { cmd = "history_clear" });

        public static JsonDocument? History(int last = 20) =>
            Send(new { cmd = "history", last });

        public static JsonDocument? RevokeTrustedDevice(string deviceId) =>
            Send(new { cmd = "revoke_trusted_device", device_id = deviceId });

        // ── Transfer Controls ─────────────────────────────────────────────────
        public static JsonDocument? AcceptFileTransfer(string transferId) => Send(new { cmd = "accept_file_transfer", transfer_id = transferId });
        public static JsonDocument? RejectFileTransfer(string transferId, string reason) => Send(new { cmd = "reject_file_transfer", transfer_id = transferId, reason = reason });
        public static JsonDocument? PauseFileTransfer(string transferId) => Send(new { cmd = "pause_file_transfer", transfer_id = transferId });
        public static JsonDocument? ResumeFileTransfer(string transferId) => Send(new { cmd = "resume_file_transfer", transfer_id = transferId });
        public static JsonDocument? CancelFileTransfer(string transferId) => Send(new { cmd = "cancel_file_transfer", transfer_id = transferId });

        // ── Device Management ─────────────────────────────────────────────────
        public static JsonDocument? RenameTrustedDevice(string deviceId, string displayName) => Send(new { cmd = "rename_trusted_device", device_id = deviceId, display_name = displayName });
        public static JsonDocument? PauseSyncPeer(string deviceId) => Send(new { cmd = "pause_sync_peer", device_id = deviceId });
        public static JsonDocument? ResumeSyncPeer(string deviceId) => Send(new { cmd = "resume_sync_peer", device_id = deviceId });
        public static JsonDocument? ForgetDevice(string deviceId) => Send(new { cmd = "forget_device", device_id = deviceId });
        public static JsonDocument? SetAutoConnect(string deviceId, bool enabled) => Send(new { cmd = "set_auto_connect", device_id = deviceId, enabled });

        // ── Activity & Settings ───────────────────────────────────────────────
        public static JsonDocument? ActivityRecent(int limit) => Send(new { cmd = "activity_recent", limit });
        public static JsonDocument? PendingRemoteClipboards() => Send(new { cmd = "pending_remote_clipboards" });
        public static JsonDocument? ApplyClipboard(string contentHash) => Send(new { cmd = "apply_clipboard", content_hash = contentHash });
        public static JsonDocument? GetMetrics() => Send(new { cmd = "get_metrics" });

        public static JsonDocument? Shutdown() => Send(new { cmd = "shutdown" });
        
        public void Dispose() { }
    }

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
