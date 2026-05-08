// ClipRelay Windows — Preferences dialog
// Modeless settings form launched from the system-tray menu.

using System;
using System.Windows.Forms;
using Microsoft.Win32;

namespace ClipRelay.Windows
{
    internal sealed class PreferencesForm : Form
    {
        // ── Controls ─────────────────────────────────────────────────────────

        private readonly CheckBox _chkEnabled       = new() { Text = "Enable clipboard syncing", Left = 16, Top = 20, Width = 280, Checked = true };
        private readonly CheckBox _chkText          = new() { Text = "Sync text",   Left = 32, Top = 50, Width = 260, Checked = true };
        private readonly CheckBox _chkImages        = new() { Text = "Sync images", Left = 32, Top = 74, Width = 260, Checked = true };
        private readonly CheckBox _chkFiles         = new() { Text = "Sync files",  Left = 32, Top = 98, Width = 260, Checked = true };
        private readonly CheckBox _chkNotifications = new() { Text = "Show notifications on receive", Left = 16, Top = 140, Width = 280, Checked = true };
        private readonly CheckBox _chkTofu          = new() { Text = "Require confirmation for new devices", Left = 16, Top = 164, Width = 280, Checked = true };
        private readonly CheckBox _chkStartOnLogin  = new() { Text = "Start ClipRelay on Windows login", Left = 16, Top = 188, Width = 280, Checked = false };

        private readonly Label    _lblDeviceName    = new() { Text = "Device name:", Left = 16, Top = 230, Width = 100 };
        private readonly TextBox  _txtDeviceName    = new() { Left = 120, Top = 227, Width = 200, PlaceholderText = "(use computer name)" };

        private readonly Label    _lblPort          = new() { Text = "Port:", Left = 16, Top = 260, Width = 100 };
        private readonly NumericUpDown _nudPort     = new() { Left = 120, Top = 258, Width = 80, Minimum = 1024, Maximum = 65535, Value = 47823 };
        private readonly Label    _lblPortNote      = new() { Text = "restart required", Left = 210, Top = 262, Width = 130, ForeColor = System.Drawing.Color.Gray };

        private readonly GroupBox _grpGeneral       = new() { Text = "General", Left = 8, Top = 4, Width = 350, Height = 130 };
        private readonly GroupBox _grpPrivacy       = new() { Text = "Privacy & Security", Left = 8, Top = 138, Width = 350, Height = 80 };
        private readonly GroupBox _grpNetwork       = new() { Text = "Network", Left = 8, Top = 222, Width = 350, Height = 100 };

        private readonly Button   _btnSave          = new() { Text = "Save", Left = 196, Top = 336, Width = 80, DialogResult = DialogResult.OK };
        private readonly Button   _btnCancel        = new() { Text = "Cancel", Left = 282, Top = 336, Width = 80, DialogResult = DialogResult.Cancel };

        // ── Registry key ─────────────────────────────────────────────────────
        private const string RegKey = @"Software\ClipRelay";

        public PreferencesForm()
        {
            Text = "ClipRelay Preferences";
            ClientSize = new System.Drawing.Size(370, 370);
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            StartPosition = FormStartPosition.CenterScreen;

            // Build group box contents
            _chkEnabled.CheckedChanged += (_, _) => UpdateEnabledState();
            _grpGeneral.Controls.AddRange(new Control[] { _chkEnabled, _chkText, _chkImages, _chkFiles });
            _grpPrivacy.Controls.AddRange(new Control[] { _chkNotifications, _chkTofu });
            _grpNetwork.Controls.AddRange(new Control[] { _lblDeviceName, _txtDeviceName, _lblPort, _nudPort, _lblPortNote });

            Controls.AddRange(new Control[] {
                _grpGeneral, _grpPrivacy, _grpNetwork, _chkStartOnLogin, _btnSave, _btnCancel
            });

            AcceptButton = _btnSave;
            CancelButton = _btnCancel;

            _btnSave.Click += OnSave;
            Load += (_, _) => LoadSettings();
        }

        // ── Load / Save ───────────────────────────────────────────────────────

        private void LoadSettings()
        {
            using var key = Registry.CurrentUser.OpenSubKey(RegKey);
            if (key == null) return;

            _chkEnabled.Checked       = (int?)key.GetValue("SyncEnabled",       1) == 1;
            _chkText.Checked          = (int?)key.GetValue("SyncText",           1) == 1;
            _chkImages.Checked        = (int?)key.GetValue("SyncImages",         1) == 1;
            _chkFiles.Checked         = (int?)key.GetValue("SyncFiles",          1) == 1;
            _chkNotifications.Checked = (int?)key.GetValue("ShowNotifications",  1) == 1;
            _chkTofu.Checked          = (int?)key.GetValue("RequireTofu",        1) == 1;
            _chkStartOnLogin.Checked  = (int?)key.GetValue("StartOnLogin",       0) == 1;
            _txtDeviceName.Text       = (string?)key.GetValue("DeviceName", "") ?? "";
            _nudPort.Value            = Math.Clamp((int?)key.GetValue("Port", 47823) ?? 47823, 1024, 65535);

            UpdateEnabledState();
        }

        private void OnSave(object? s, EventArgs e)
        {
            using var key = Registry.CurrentUser.CreateSubKey(RegKey);
            key.SetValue("SyncEnabled",       _chkEnabled.Checked       ? 1 : 0, RegistryValueKind.DWord);
            key.SetValue("SyncText",          _chkText.Checked          ? 1 : 0, RegistryValueKind.DWord);
            key.SetValue("SyncImages",        _chkImages.Checked        ? 1 : 0, RegistryValueKind.DWord);
            key.SetValue("SyncFiles",         _chkFiles.Checked         ? 1 : 0, RegistryValueKind.DWord);
            key.SetValue("ShowNotifications", _chkNotifications.Checked ? 1 : 0, RegistryValueKat.DWord);
            key.SetValue("RequireTofu",       _chkTofu.Checked          ? 1 : 0, RegistryValueKind.DWord);
            key.SetValue("DeviceName",        _txtDeviceName.Text,                RegistryValueKind.String);
            key.SetValue("Port",              (int)_nudPort.Value,                RegistryValueKind.DWord);

            ApplyLoginItem(_chkStartOnLogin.Checked);
        }

        private void UpdateEnabledState()
        {
            bool on = _chkEnabled.Checked;
            _chkText.Enabled   = on;
            _chkImages.Enabled = on;
            _chkFiles.Enabled  = on;
        }

        private static void ApplyLoginItem(bool enable)
        {
            const string runKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
            using var key = Registry.CurrentUser.OpenSubKey(runKey, writable: true);
            if (key == null) return;
            if (enable)
            {
                var exePath = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
                if (exePath != null) key.SetValue("ClipRelay", $"\"{exePath}\"");
            }
            else
            {
                key.DeleteValue("ClipRelay", throwOnMissingValue: false);
            }
        }
    }
}
