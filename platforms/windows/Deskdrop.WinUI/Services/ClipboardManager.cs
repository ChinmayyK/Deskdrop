using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Collections.ObjectModel;
using System.Collections.Generic;
using Microsoft.UI.Dispatching;
using Windows.ApplicationModel.DataTransfer;
using Deskdrop.WinUI.Views;
using System.Text.Json;
using Microsoft.UI.Xaml;

namespace Deskdrop.WinUI.Services
{
    public class ClipboardManager : IDisposable
    {
        private readonly DispatcherQueue _dispatcher;
        private string _lastText = string.Empty;
        private readonly DispatcherTimer _pollTimer;

        public ObservableCollection<HistoryItem> History { get; set; } = new ObservableCollection<HistoryItem>();

        public ClipboardManager()
        {
            _dispatcher = DispatcherQueue.GetForCurrentThread();
            Windows.ApplicationModel.DataTransfer.Clipboard.ContentChanged += Clipboard_ContentChanged;

            _pollTimer = new DispatcherTimer();
            _pollTimer.Interval = TimeSpan.FromMilliseconds(30);
            _pollTimer.Tick += OnPollTick;
            _pollTimer.Start();
        }



        private async void Clipboard_ContentChanged(object? sender, object e)
        {
            await CheckClipboardAsync();
        }

        private void OnPollTick(object? sender, object e)
        {
            DrainEvents();
        }

        private void DrainEvents()
        {
            if (App.EngineHandle == IntPtr.Zero) return;
            bool processedAny = false;
            while (true)
            {
                var ev = NativeCore.deskdrop_poll_event(App.EngineHandle);
                if (ev == IntPtr.Zero) break;
                processedAny = true;
                try
                {
                    int kind = NativeCore.deskdrop_event_type(ev);
                    switch (kind)
                    {
                        case NativeCore.PB_EVENT_CLIPBOARD_TEXT:
                        {
                            var text = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_text(ev));
                            var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                            if (text != null)
                            {
                                _lastText = text;
                                (_dispatcher ?? App.MainDispatcherQueue)?.TryEnqueue(() => {
                                    try {
                                        var package = new DataPackage();
                                        package.SetText(text);
                                        Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
                                    } catch (Exception ex) { App.HandleError(ex); }
                                    AddHistoryItem(text, from, "📝", text);
                                });
                            }
                            break;
                        }
                        case NativeCore.PB_EVENT_FILE_TRANSFER_INCOMING:
                        case NativeCore.PB_EVENT_CLIPBOARD_FILE:
                        {
                            var fileName = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_transfer_file_name(ev)) ?? "File";
                            var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                            var transferId = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_transfer_id(ev)) ?? "";
                            (_dispatcher ?? App.MainDispatcherQueue)?.TryEnqueue(() => {
                                AddHistoryItem(fileName, from, "📎", fileName);
                                NotificationHelper.ShowToastWithActions(
                                    $"Incoming File from {from}",
                                    fileName,
                                    null,
                                    $"deskdrop://accept/{transferId}",
                                    $"deskdrop://reject/{transferId}"
                                );
                            });
                            break;
                        }
                        case NativeCore.PB_EVENT_CALL_STATE_CHANGED:
                        {
                            var from = NativeCore.PtrToUtf8String(NativeCore.deskdrop_event_device_name(ev)) ?? "Unknown";
                            (_dispatcher ?? App.MainDispatcherQueue)?.TryEnqueue(() => {
                                try { new IncomingCallBannerWindow().Activate(); } catch (Exception ex) { App.HandleError(ex); }
                            });
                            break;
                        }
                    }
                }
                catch (Exception ex) { App.HandleError(ex); }
                finally
                {
                    NativeCore.deskdrop_free_event(ev);
                }
            }
            if (processedAny)
            {
                DeskdropStore.Shared.UpdateStateFromDaemon();
            }
        }

        private void AddHistoryItem(string summary, string source, string icon, string fullText)
        {
            var item = new HistoryItem
            {
                Summary = summary.Length > 80 ? summary[..77] + "…" : summary,
                FullText = fullText,
                Source = source,
                TypeIcon = icon,
                Time = DateTime.Now,
                RelativeTime = "Just now",
                display_text = summary,
                is_text = icon == "📝"
            };
            History.Insert(0, item);
            if (History.Count > 100) History.RemoveAt(History.Count - 1);
            try { DeskdropStore.Shared.History.Insert(0, item); } catch (Exception ex) { App.HandleError(ex); }
        }

        private async Task CheckClipboardAsync()
        {
            try
            {
                var packageView = global::Windows.ApplicationModel.DataTransfer.Clipboard.GetContent();
                if (packageView == null) return;
                
                if (packageView.Contains(StandardDataFormats.Text))
                {
                    var text = await packageView.GetTextAsync();
                    if (!string.IsNullOrEmpty(text) && text != _lastText)
                    {
                        if (DeskdropStore.Shared.OtpShieldEnabled && IsSensitiveContent(text))
                        {
                            _lastText = text;
                            return; // Filter out sensitive OTP/passwords/tokens
                        }
                        _lastText = text;
                        if (App.EngineHandle != IntPtr.Zero)
                        {
                            NativeCore.deskdrop_push_text(App.EngineHandle, text);
                        }
                        else
                        {
                            DaemonClient.PushText(text);
                        }
                        AddHistoryItem(text, "local", "📝", text);
                    }
                }
                else if (packageView.Contains(StandardDataFormats.StorageItems))
                {
                    var items = await packageView.GetStorageItemsAsync();
                    foreach (var item in items)
                    {
                        if (item is Windows.Storage.StorageFile file)
                        {
                            try {
                                if (App.EngineHandle != IntPtr.Zero)
                                {
                                    NativeCore.deskdrop_send_file_path(App.EngineHandle, null, file.Path, file.Name, "application/octet-stream");
                                }
                                else
                                {
                                    DaemonClient.SendFilePath(file.Path, file.Name, "application/octet-stream");
                                }
                                AddHistoryItem(file.Name, "local", "📎", file.Path);
                            } catch (Exception ex) { App.HandleError(ex); }
                        }
                    }
                }
            }
            catch (Exception ex) { App.HandleError(ex); }
        }

        public void HandleIncomingData(System.Text.Json.JsonElement json)
        {
            if (json.TryGetProperty("type", out var typeProp) && typeProp.GetString() == "file")
            {
                string name = json.GetProperty("name").GetString() ?? "Unknown";
                string from = json.GetProperty("from").GetString() ?? "Unknown";
                _dispatcher?.TryEnqueue(() => {
                    AddHistoryItem(name, from, "📎", name);
                    NotificationHelper.ShowToastWithActions(
                        $"Incoming File from {from}",
                        name,
                        null,
                        "deskdrop://accept/0",
                        "deskdrop://reject/0"
                    );
                });
            }
        }

        public void PushFile(string path, string? targetDevice = null)
        {
            if (!string.IsNullOrEmpty(path) && File.Exists(path))
            {
                string name = Path.GetFileName(path);
                try {
                    if (App.EngineHandle != IntPtr.Zero)
                    {
                        NativeCore.deskdrop_send_file_path(App.EngineHandle, targetDevice, path, name, "application/octet-stream");
                    }
                    else
                    {
                        DaemonClient.SendFilePath(path, name, "application/octet-stream", targetDevice);
                    }
                    _dispatcher.TryEnqueue(() => AddHistoryItem(name, "local", "📎", path));
                } catch (Exception ex) { App.HandleError(ex); }
            }
        }

        public void PushLocalClipboard()
        {
        }

        private bool IsSensitiveContent(string text)
        {
            if (string.IsNullOrWhiteSpace(text)) return false;
            var t = text.Trim();
            if (t.Length >= 4 && t.Length <= 8 && t.All(char.IsDigit)) return true; // 4-8 digit OTP code
            if (t.StartsWith("Bearer ", StringComparison.OrdinalIgnoreCase)) return true;
            if (t.StartsWith("sk-", StringComparison.OrdinalIgnoreCase)) return true;
            if (t.Contains("password", StringComparison.OrdinalIgnoreCase)) return true;
            return false;
        }

        public void Dispose()
        {
            Windows.ApplicationModel.DataTransfer.Clipboard.ContentChanged -= Clipboard_ContentChanged;
            _pollTimer.Stop();
        }
    }
}

