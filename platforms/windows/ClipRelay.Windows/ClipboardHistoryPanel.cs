// ClipboardHistoryPanel.cs
// Windows Forms panel shown in a floating ToolStripDropDown from the tray icon.

using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Forms;

namespace ClipRelay.Windows
{
    internal sealed class HistoryItem
    {
        public int     Id        { get; init; }
        public string  Summary   { get; init; } = "";
        public string  Source    { get; init; } = "local";
        public DateTime Time     { get; init; }
        public string? FullText  { get; init; }   // null for images/files
        public string  TypeIcon  { get; init; } = "📄";

        public string RelativeTime
        {
            get
            {
                var s = (int)(DateTime.Now - Time).TotalSeconds;
                if (s < 60)   return $"{s}s ago";
                if (s < 3600) return $"{s/60}m ago";
                return $"{s/3600}h ago";
            }
        }
    }

    internal sealed class ClipboardHistoryPanel : UserControl
    {
        // ── Controls ─────────────────────────────────────────────────────────

        private readonly TextBox   _search     = new() { PlaceholderText = "Search history…", Dock = DockStyle.Top, Height = 28, BorderStyle = BorderStyle.None };
        private readonly ListView  _list       = new() { Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true, GridLines = false, BorderStyle = BorderStyle.None, ShowItemToolTips = true };
        private readonly Panel     _footer     = new() { Dock = DockStyle.Bottom, Height = 30, BackColor = SystemColors.Control };
        private readonly Label     _countLabel = new() { AutoSize = true };
        private readonly Button    _clearBtn   = new() { Text = "Clear All", FlatStyle = FlatStyle.Flat, Height = 24, Width = 70 };

        private List<HistoryItem> _allItems  = new();
        private List<HistoryItem> _displayed = new();

        // ── Events ────────────────────────────────────────────────────────────

        /// Fired when user wants to re-push an item to peers.
        public event Action<HistoryItem>? RepushRequested;

        // ── Constructor ───────────────────────────────────────────────────────

        public ClipboardHistoryPanel()
        {
            Size = new Size(380, 480);
            BackColor = SystemColors.Window;

            // Search bar panel.
            var searchPanel = new Panel { Dock = DockStyle.Top, Height = 36, Padding = new Padding(6, 4, 6, 4) };
            _search.Dock = DockStyle.Fill;
            _search.TextChanged += (_, _) => ApplyFilter();
            searchPanel.Controls.Add(_search);

            // List columns.
            _list.Columns.Add("", 28);          // icon
            _list.Columns.Add("Summary", 230);
            _list.Columns.Add("Source", 70);
            _list.Columns.Add("Time", 50);
            _list.DoubleClick += OnRepush;
            _list.KeyDown += (_, e) => { if (e.KeyCode == Keys.Return) OnRepush(null, EventArgs.Empty); };

            // Footer.
            _clearBtn.Dock = DockStyle.Right;
            _clearBtn.ForeColor = Color.Crimson;
            _clearBtn.Click += (_, _) => ClearHistory();
            _countLabel.Dock = DockStyle.Left;
            _countLabel.Padding = new Padding(6, 0, 0, 0);
            _footer.Controls.Add(_clearBtn);
            _footer.Controls.Add(_countLabel);

            var separator1 = new Panel { Dock = DockStyle.Top, Height = 1, BackColor = SystemColors.ControlLight };
            var separator2 = new Panel { Dock = DockStyle.Bottom, Height = 1, BackColor = SystemColors.ControlLight };

            Controls.Add(_list);
            Controls.Add(separator1);
            Controls.Add(searchPanel);
            Controls.Add(separator2);
            Controls.Add(_footer);

            Load();
        }

        // ── Public API ────────────────────────────────────────────────────────

        public void AddItem(HistoryItem item)
        {
            // Deduplicate: remove earlier entry with same text.
            _allItems.RemoveAll(i => i.FullText != null && i.FullText == item.FullText);
            _allItems.Insert(0, item);

            // Cap at 100.
            if (_allItems.Count > 100) _allItems.RemoveRange(100, _allItems.Count - 100);
            ApplyFilter();
        }

        // ── Private ───────────────────────────────────────────────────────────

        private void Load()
        {
            // In production: call `cliprelay-cli history --last 50` or IPC.
            _allItems = LoadFromCli();
            ApplyFilter();
        }

        private List<HistoryItem> LoadFromCli()
        {
            try
            {
                var result = RunCli("history --last 50");
                // Parse JSON; return parsed list.
                // For now return empty — real impl parses cliprelay-cli JSON.
                return new();
            }
            catch { return new(); }
        }

        private void ApplyFilter()
        {
            var q = _search.Text.Trim().ToLowerInvariant();
            _displayed = string.IsNullOrEmpty(q)
                ? new List<HistoryItem>(_allItems)
                : _allItems.FindAll(i =>
                    i.Summary.ToLowerInvariant().Contains(q) ||
                    i.Source.ToLowerInvariant().Contains(q));

            _list.BeginUpdate();
            _list.Items.Clear();
            foreach (var item in _displayed)
            {
                var lvi = new ListViewItem(item.TypeIcon);
                lvi.SubItems.Add(item.Summary.Length > 50 ? item.Summary[..47] + "…" : item.Summary);
                lvi.SubItems.Add(item.Source == "local" ? "me" : item.Source);
                lvi.SubItems.Add(item.RelativeTime);
                lvi.ToolTipText = item.FullText ?? item.Summary;
                lvi.Tag = item;
                _list.Items.Add(lvi);
            }
            _list.EndUpdate();
            _countLabel.Text = $"  {_displayed.Count} items";
        }

        private void OnRepush(object? sender, EventArgs e)
        {
            if (_list.SelectedItems.Count == 0) return;
            var item = (HistoryItem)_list.SelectedItems[0].Tag!;
            RepushRequested?.Invoke(item);
        }

        private void ClearHistory()
        {
            var r = MessageBox.Show("Clear all clipboard history?", "Confirm",
                MessageBoxButtons.YesNo, MessageBoxIcon.Question);
            if (r != DialogResult.Yes) return;
            RunCli("history clear");
            _allItems.Clear();
            ApplyFilter();
        }

        private static string RunCli(string args)
        {
            var p = new System.Diagnostics.Process();
            p.StartInfo = new System.Diagnostics.ProcessStartInfo
            {
                FileName  = "cliprelay-cli.exe",
                Arguments = args,
                RedirectStandardOutput = true,
                UseShellExecute = false,
                CreateNoWindow  = true,
            };
            p.Start();
            string output = p.StandardOutput.ReadToEnd();
            p.WaitForExit();
            return output;
        }
    }
}
