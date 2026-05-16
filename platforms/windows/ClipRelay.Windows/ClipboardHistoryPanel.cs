// ClipboardHistoryPanel.cs — floating clipboard history panel
// Populated from ClipboardManager.GetHistory() / HistoryItemAdded events.
// Shown near the system tray (bottom-right), triggered by double-click or menu. (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
using System.Windows.Forms;

namespace ClipRelay.Windows
{
    internal sealed class HistoryItem
    {
        public int      Id       { get; init; } = Environment.TickCount;
        public string   Summary  { get; init; } = "";
        public string   Source   { get; init; } = "local";
        public DateTime Time     { get; init; } = DateTime.Now;
        public string?  FullText { get; init; }
        public string   TypeIcon { get; init; } = "📄"; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

        public string RelativeTime
        {
            get
            {
                int s = (int)(DateTime.Now - Time).TotalSeconds;
                return s < 60 ? $"{s}s ago"
                    : s < 3600 ? $"{s / 60}m ago"
                    : $"{s / 3600}h ago"; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
            }
        }
    }

    internal sealed class ClipboardHistoryPanel : Form
    {
        // ── Controls ─────────────────────────────────────────────────────────

        private readonly TextBox   _search;
        private readonly ListView  _list;
        private readonly Label     _countLabel;
        private readonly Button    _clearBtn; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

        private List<HistoryItem> _allItems  = new();
        private List<HistoryItem> _displayed = new();

        // ── Events ────────────────────────────────────────────────────────────

        /// Fired when the user double-clicks or presses Enter — caller pushes the item. (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
        public event Action<HistoryItem>? RepushRequested;

        // ── Constructor ───────────────────────────────────────────────────────

        public ClipboardHistoryPanel()
        {
            Text             = "ClipRelay — Clipboard History";
            ClientSize       = new Size(420, 520);
            FormBorderStyle  = FormBorderStyle.SizableToolWindow;
            StartPosition    = FormStartPosition.Manual;
            ShowInTaskbar    = false;
            TopMost          = true;
            MinimumSize      = new Size(320, 300);
            BackColor        = SystemColors.Window;

            // ── Search bar ────────────────────────────────────────────────────
            var searchPanel = new Panel
            {
                Dock = DockStyle.Top, Height = 40, Padding = new Padding(6, 6, 6, 0),
                BackColor = SystemColors.Control,
            };
            _search = new TextBox
            {
                Dock = DockStyle.Fill, PlaceholderText = "🔍  Search history…",
                BorderStyle = BorderStyle.FixedSingle,
            };
            _search.TextChanged += (_, _) => ApplyFilter();
            searchPanel.Controls.Add(_search);

            // ── List ──────────────────────────────────────────────────────────
            _list = new ListView
            {
                Dock           = DockStyle.Fill,
                View           = View.Details,
                FullRowSelect  = true,
                GridLines      = false,
                BorderStyle    = BorderStyle.None,
                ShowItemToolTips = true,
                HideSelection  = false,
            };
            _list.Columns.Add("",       28);   // icon
            _list.Columns.Add("Text",  240);   // summary
            _list.Columns.Add("From",   68);   // source device
            _list.Columns.Add("Time",   58);   // relative time
            _list.ColumnWidthChanged += (_, _) => { }; // allow resize
            _list.DoubleClick += OnRepush;
            _list.KeyDown     += (_, e) =>
            {
                if (e.KeyCode == Keys.Return)  { OnRepush(null, EventArgs.Empty); e.Handled = true; }
                if (e.KeyCode == Keys.Delete)  { DeleteSelected(); e.Handled = true; }
                if (e.KeyCode == Keys.Escape)  { Close(); e.Handled = true; }
            };

            // Right-click context menu on list items.
            var listMenu = new ContextMenuStrip();
            listMenu.Items.Add("📋 Apply to clipboard",   null, (_, _) => ApplySelected());
            listMenu.Items.Add("📤 Send to devices",      null, (_, _) => OnRepush(null, EventArgs.Empty));
            listMenu.Items.Add(new ToolStripSeparator());
            listMenu.Items.Add("🗑️ Delete",              null, (_, _) => DeleteSelected());
            _list.ContextMenuStrip = listMenu;

            // ── Footer ────────────────────────────────────────────────────────
            var footer = new Panel
            {
                Dock = DockStyle.Bottom, Height = 36,
                BackColor = SystemColors.Control, Padding = new Padding(6, 4, 6, 4),
            };
            _countLabel = new Label
            {
                Dock = DockStyle.Left, AutoSize = false, Width = 130,
                TextAlign = ContentAlignment.MiddleLeft, ForeColor = SystemColors.GrayText,
            };
            _clearBtn = new Button
            {
                Dock = DockStyle.Right, Width = 90, Text = "Clear All",
                FlatStyle = FlatStyle.Flat, ForeColor = Color.Crimson,
            };
            _clearBtn.Click += (_, _) => ClearAll();
            footer.Controls.Add(_clearBtn);
            footer.Controls.Add(_countLabel);

            var sep1 = new Panel { Dock = DockStyle.Top,    Height = 1, BackColor = SystemColors.ControlLight };
            var sep2 = new Panel { Dock = DockStyle.Bottom, Height = 1, BackColor = SystemColors.ControlLight };

            Controls.Add(_list);
            Controls.Add(sep1);
            Controls.Add(searchPanel);
            Controls.Add(sep2);
            Controls.Add(footer);

            // Escape closes when focus is on the form (not inside a text box).
            KeyPreview = true;
            KeyDown   += (_, e) => { if (e.KeyCode == Keys.Escape) Close(); }; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
        }

        // ── Public API ────────────────────────────────────────────────────────

        /// Add a new item (called from ClipboardManager.HistoryItemAdded event).
        public void AddItem(HistoryItem item)
        {
            if (InvokeRequired) { BeginInvoke(() => AddItem(item)); return; }
            _allItems.RemoveAll(i => i.FullText != null && i.FullText == item.FullText);
            _allItems.Insert(0, item); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
            if (_allItems.Count > 100) _allItems.RemoveRange(100, _allItems.Count - 100);
            ApplyFilter();
        }

        // ── Private ───────────────────────────────────────────────────────────

        private void ApplyFilter()
        {
            string q = _search.Text.Trim().ToLowerInvariant();
            _displayed = string.IsNullOrEmpty(q)
                ? _allItems.ToList()
                : _allItems.Where(i =>
                    i.Summary.ToLowerInvariant().Contains(q) ||
                    i.Source.ToLowerInvariant().Contains(q) ||
                    (i.FullText?.ToLowerInvariant().Contains(q) ?? false)).ToList(); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)

            _list.BeginUpdate();
            _list.Items.Clear();
            foreach (var item in _displayed)
            {
                var lvi = new ListViewItem(item.TypeIcon);
                lvi.SubItems.Add(item.Summary.Length > 60 ? item.Summary[..57] + "…" : item.Summary); (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
                lvi.SubItems.Add(item.Source == "local" ? "me" : item.Source);
                lvi.SubItems.Add(item.RelativeTime);
                lvi.ToolTipText = item.FullText ?? item.Summary;
                lvi.Tag = item;

                // Colour-code by direction.
                lvi.ForeColor = item.Source == "local"
                    ? SystemColors.ControlText
                    : Color.FromArgb(0, 100, 200);
                _list.Items.Add(lvi);
            }
            _list.EndUpdate();
            _countLabel.Text = _displayed.Count == 0 ? "No items"
                : $"{_displayed.Count} item{(_displayed.Count == 1 ? "" : "s")}"; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
        }

        private void OnRepush(object? sender, EventArgs e)
        {
            var item = SelectedItem();
            if (item != null) RepushRequested?.Invoke(item);
        }

        private void ApplySelected()
        {
            var item = SelectedItem();
            if (item?.FullText == null) return;
            var t = new Thread(() =>
            {
                try { Clipboard.SetText(item.FullText); }
                catch { }
            });
            t.SetApartmentState(ApartmentState.STA);
            t.IsBackground = true;
            t.Start();
        }

        private void DeleteSelected()
        {
            var item = SelectedItem();
            if (item == null) return;
            _allItems.RemoveAll(i => i.Id == item.Id);
            ApplyFilter();
        }

        private void ClearAll()
        {
            if (MessageBox.Show("Clear all clipboard history?", "Confirm",
                    MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
            _allItems.Clear();
            ApplyFilter();
        }

        private HistoryItem? SelectedItem() =>
            _list.SelectedItems.Count > 0 ? _list.SelectedItems[0].Tag as HistoryItem : null; (feat: enhance core daemon, FFI, and IPC; major updates to Windows and Linux platform implementations)
    }
}
