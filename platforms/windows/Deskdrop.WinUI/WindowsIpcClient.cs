using Microsoft.UI.Dispatching;
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

namespace Deskdrop.WinUI
{
    /// <summary>
    /// Named-pipe client that talks to the running Deskdrop daemon.
    /// Thread-safe: each request opens a fresh pipe connection.
    /// </summary>
    internal sealed class DaemonClient : IDisposable
    {
        private static string PipeName
        {
            get
            {
                var localAppData = Environment.GetEnvironmentVariable("LOCALAPPDATA") ?? "default";
                var sanitized = localAppData.Replace('\\', '_').Replace(':', '_');
                return "deskdrop_" + sanitized;
            }
        }
        private const int    TimeoutMs   = 1000;


        public static JsonDocument? SendFilePath(string path, string name, string mime, string? targetDevice = null, string? batchId = null, bool isDirectory = false, int itemCount = 1)
        {
            var req = new { cmd = "send_file_path", path = path, name = name, mime = mime, target_device = targetDevice, batch_id = batchId, is_directory = isDirectory, item_count = itemCount };
            return Send(req);
        }

        public static JsonDocument? PushFile(string targetDevice, string path)
        {
            var fileName = System.IO.Path.GetFileName(path);
            return SendFilePath(path, fileName, "application/octet-stream", string.IsNullOrEmpty(targetDevice) ? null : targetDevice);
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
                var line = ReadLineWithTimeout(pipe, TimeoutMs);
                if (line != null)
                {
                    try { return JsonDocument.Parse(line); } catch { return null; }
                }
                return null;
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

        public static JsonDocument? PushTextTo(string text, string targetDevice) =>
            Send(new { cmd = "push_text_to", text, target = targetDevice });

        public static JsonDocument? PushClipboard(string? targetDeviceId = null) =>
            Send(new { cmd = "push_clipboard", target_device_id = targetDeviceId });

        public static JsonDocument? SetSyncEnabled(bool enabled) =>
            Send(new { cmd = "set_sync_enabled", enabled });

        public static JsonDocument? PushBatteryStatus(int level, bool charging) =>
            Send(new { cmd = "push_battery_status", level, charging });

        public static JsonDocument? PushStorageStatus(ulong imagesBytes, ulong videosBytes, ulong appsBytes, ulong freeBytes, ulong totalBytes) =>
            Send(new { 
                cmd = "push_storage_status", 
                images_bytes = imagesBytes, 
                videos_bytes = videosBytes, 
                apps_bytes = appsBytes, 
                free_bytes = freeBytes, 
                total_bytes = totalBytes 
            });

        public static JsonDocument? HistoryClear() => Send(new { cmd = "history_clear" });

        public static JsonDocument? History(int last = 20) =>
            Send(new { cmd = "history", last });

        public static JsonDocument? RevokeTrustedDevice(string deviceId) =>
            Send(new { cmd = "revoke_trusted_device", device_id = deviceId });

        // ── Transfer Controls ─────────────────────────────────────────────────
        public static JsonDocument? SendPairingRequest(string deviceId) => Send(new { cmd = "send_pairing_request", device_id = deviceId });
        public static JsonDocument? RespondToPairing(string deviceId, bool accepted) => Send(new { cmd = "respond_to_pairing", device_id = deviceId, accepted });
        public static JsonDocument? AcceptFileTransfer(string transferId) => Send(new { cmd = "accept_file_transfer", transfer_id = transferId });
        public static JsonDocument? RejectFileTransfer(string transferId, string reason) => Send(new { cmd = "reject_file_transfer", transfer_id = transferId, reason = reason });
        public static JsonDocument? PauseFileTransfer(string transferId) => Send(new { cmd = "pause_file_transfer", transfer_id = transferId });
        public static JsonDocument? ResumeFileTransfer(string transferId) => Send(new { cmd = "resume_file_transfer", transfer_id = transferId });
        public static JsonDocument? CancelFileTransfer(string transferId) => Send(new { cmd = "cancel_file_transfer", transfer_id = transferId });
        public static JsonDocument? StartSpeedTest(string deviceId, int durationSecs = 10) => Send(new { cmd = "start_speed_test", device_id = deviceId, duration_secs = durationSecs });

        // ── Device Management ─────────────────────────────────────────────────
        public static JsonDocument? DisconnectPeer(string deviceId) => Send(new { cmd = "disconnect_peer", device_id = deviceId });
        public static JsonDocument? DisconnectAllPeers()
        {
            try
            {
                var peers = DeskdropStore.Shared.Peers;
                System.Collections.Generic.List<string> ids = new();
                if (App.MainDispatcherQueue?.HasThreadAccess == true)
                {
                    foreach (var p in peers) ids.Add(p.device_id);
                }
                else
                {
                    // If on background thread, send explicit IPC command or safely marshal
                    Send(new { cmd = "disconnect_all_peers" });
                    return null;
                }
                foreach (var id in ids) DisconnectPeer(id);
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
            return null;
        }
        public static JsonDocument? RescanPeers() => Send(new { cmd = "rescan_peers" });
        public static JsonDocument? RenameTrustedDevice(string deviceId, string displayName) => Send(new { cmd = "rename_trusted_device", device_id = deviceId, display_name = displayName });
        public static JsonDocument? PauseSyncPeer(string deviceId) => Send(new { cmd = "pause_sync_peer", device_id = deviceId });
        public static JsonDocument? ResumeSyncPeer(string deviceId) => Send(new { cmd = "resume_sync_peer", device_id = deviceId });
        public static JsonDocument? ForgetDevice(string deviceId) => Send(new { cmd = "forget_device", device_id = deviceId });
        public static JsonDocument? SetAutoConnect(string deviceId, bool enabled) => Send(new { cmd = "set_auto_connect", device_id = deviceId, enabled });

        // ── Activity & Settings ───────────────────────────────────────────────
        public static JsonDocument? ActivityRecent(int limit) => Send(new { cmd = "activity_recent", limit });
        public static JsonDocument? PendingRemoteClipboards() => Send(new { cmd = "pending_remote_clipboards" });
        public static JsonDocument? ApplyClipboard(string contentHash) => Send(new { cmd = "apply_clipboard", content_hash = contentHash });
        public static JsonDocument? GetSettings() => Send(new { cmd = "get_settings" });
        public static JsonDocument? GetMetrics() => Send(new { cmd = "get_metrics" });

        // ── Remote File Explorer ──────────────────────────────────────────────
        public static async Task<JsonDocument?> RemoteFilesQueryAsync(string deviceId, bool summaryOnly = false, string? category = null, string? source = null, string? searchQuery = null, uint offset = 0, uint limit = 100)
        {
            var req = new System.Collections.Generic.Dictionary<string, object>
            {
                ["cmd"] = "remote_files_query",
                ["target_device"] = deviceId,
                ["summary_only"] = summaryOnly,
                ["offset"] = offset,
                ["limit"] = limit
            };
            if (!string.IsNullOrEmpty(category)) req["category"] = category;
            if (!string.IsNullOrEmpty(source)) req["source"] = source;
            if (!string.IsNullOrEmpty(searchQuery)) req["search_query"] = searchQuery;
            return await SendAsync(req);
        }
            
        public static JsonDocument? RemoteFilePullRequest(string deviceId, ulong fileId) =>
            Send(new { cmd = "remote_file_pull_request", target_device = deviceId, file_id = fileId });

        public static JsonDocument? RemoteFileActionRequest(string deviceId, ulong fileId, string action, string? newName = null)
        {
            var req = new System.Collections.Generic.Dictionary<string, object>
            {
                ["cmd"] = "remote_file_action_request",
                ["target_device"] = deviceId,
                ["file_id"] = fileId,
                ["action"] = action
            };
            if (!string.IsNullOrEmpty(newName)) req["new_name"] = newName;
            return Send(req);
        }

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
            try
            {
                bool running = DaemonClient.IsDaemonRunning();
                if (running != _wasDaemonRunning)
                {
                    _wasDaemonRunning = running;
                    try { DaemonAvailabilityChanged?.Invoke(running); } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
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
                        { _lastPeerCount = peerCount; try { PeerCountChanged?.Invoke(peerCount); } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); } }
                        if (syncEnabled != _lastSyncState)
                        { _lastSyncState = syncEnabled; try { SyncStateChanged?.Invoke(syncEnabled); } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); } }
                        if (pending != _lastPendingClipboard)
                        { _lastPendingClipboard = pending; try { PendingClipboardCountChanged?.Invoke(pending); } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); } }
                    }
                }
                catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }

                // Adaptive interval: fast when peers are present, slow otherwise.
                SchedulePoll(_lastPeerCount > 0 ? FastMs : SlowMs);
            }
            catch
            {
                try { SchedulePoll(SlowMs); } catch (Exception ex) { System.Diagnostics.Debug.WriteLine($"Swallowed Exception: {ex.Message}\n{ex.StackTrace}"); }
            }
        }

        public void Dispose() { _timer?.Dispose(); }
    }
}









