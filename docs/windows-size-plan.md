# Windows app size plan

Handoff notes for shrinking the Windows (WinUI 3) app. Status as of 30 September 2026.

## Goal and constraints

- Get the installed app from ~296 MB to ~70–90 MB, self-contained, with a ~30–40 MB download.
- Stay on WinUI 3 (decided: no WPF/Tauri/Slint rewrite).
- Keep the per-user MSI that installs **without admin rights**. That rules out framework-dependent .NET: the .NET Desktop Runtime installer is machine-wide and needs admin.

## Where the size went (measured, Release build before this work)

| Part | Size |
|---|---|
| Stale `publish\tray` copy written by every plain build (never shipped) | 113 MB |
| `tray\`: WinForms helper with its own self-contained .NET runtime | 112 MB |
| Main app top-level files (.NET 8 runtime ≈70 MB, Windows App SDK/WinUI ≈55 MB, rest) | 177 MB |
| Deskdrop's own code (`Deskdrop.dll` 0.9 MB, `deskdrop_core.dll` 4.3 MB) | ~5 MB |

Largest single files: `Microsoft.Windows.SDK.NET.dll` 25 MB, `Microsoft.ui.xaml.dll` 14.6 MB, `System.Private.CoreLib.dll` 12.6 MB, `System.Private.Xml.dll` 7.6 MB, `Microsoft.WinUI.dll` 7 MB.

Already handled before this plan: the csproj deletes the unused Windows AI runtime (~42 MB). ReadyToRun was measured at <1 MB of difference, so it is not a lever.

## Done

### Phase 1: clean base (commit `2b8e301`)

- Publish-folder tray copy moved to a Publish-only target: no more stale 113 MB copy on plain builds.
- `SatelliteResourceLanguages=en`; a `PruneWinUiLocales` target keeps only the `en-us` WinUI `.mui` folder on publish. Side effect: built-in control text is English on non-English Windows.
- `platforms/windows/installer/Deskdrop.wxs` rewritten for WiX 5: installs the whole publish folder via `<Files>`, `CompressionLevel="high"`, registry version from `[ProductVersion]`. Still per-user.
- `.github/workflows/release.yml`: folder publish (not single-file), checks `Deskdrop.exe`, `deskdrop_core.dll` and `tray\Deskdrop.Tray.exe` exist, builds `Deskdrop-windows-x64.msi` and `Deskdrop-windows-x64.zip`, attaches both to the release.
- `scripts/windows-size.ps1 <publishDir> [msi]`: size report.

### Phase 2: native tray (commit `64c02c4`)

- `platforms/windows/tray`: Rust Win32 tray (`windows-sys`), release exe ~312 KB, in the Cargo workspace.
- Same contract as the old WinForms helper, so `TrayService.cs` barely changed:
  - runs as `tray\Deskdrop.Tray.exe` (watchdog checks that process name);
  - reads one line per connection on `\\.\pipe\Deskdrop_Tray_Pipe`: `TIP:<text>`, `NOTIFY:<text>`, `QUIT`;
  - writes `QUICKACCESS`, `SETTINGS`, `RESCAN` to `\\.\pipe\Deskdrop_Main_Commands`, falling back to opening the app;
  - re-adds the icon on `TaskbarCreated` (Explorer restart); logs to `%LOCALAPPDATA%\Deskdrop\tray_debug.txt`.
- `Deskdrop.WinUI.csproj` runs `cargo build --release -p deskdrop-tray` before Build (skip with `-p:SkipTrayBuild=true`) and copies the exe into `tray\` on build and publish.
- WinForms `Deskdrop.Tray` project deleted. "Rescan Network" fixed (it posted to a localhost HTTP port nothing listened on).

Also in `137ee8c`: workspace-wide `cargo fmt`, and a clippy fix in `deskdrop-core/src/engine/listener.rs` (test module moved to the end of the file) so the CI gates pass.

### Verified so far

- Tray: builds, clippy `-D warnings` clean, smoke-tested over the pipe (survives a watchdog probe and `TIP:`, exits on `QUIT`).
- **Not verified:** nothing on the WinUI side has been built — the original machine had no .NET SDK. The WiX `<Files>` syntax is the most likely thing to fail.

## First steps on the new machine

1. Install: .NET 8 SDK (for phases 1–2 as committed), .NET 10 SDK (phase 3), Rust stable with the MSVC or GNU toolchain, WiX 5 (`dotnet tool install -g wix --version 5.*`). Check with `dotnet --info`.
2. Build the core: `cargo build --release -p deskdrop-core`.
3. Publish and measure (from the repo root):
   ```
   dotnet publish platforms\windows\Deskdrop.WinUI\Deskdrop.WinUI.csproj -c Release -r win-x64 --self-contained true -o publish\windows
   pwsh scripts\windows-size.ps1 publish\windows
   wix build platforms\windows\installer\Deskdrop.wxs -bindpath App=publish\windows -bindpath platforms\windows\installer -o Deskdrop-windows-x64.msi
   pwsh scripts\windows-size.ps1 publish\windows Deskdrop-windows-x64.msi
   ```
   Expected: ~185 MB installed (estimate, not yet measured).
4. Test phases 1–2 before starting phase 3:
   - Install the MSI as a **non-admin** user; app starts; uninstall removes the folder.
   - Tray icon appears; every menu item works (Open, Quick Access, Settings, Rescan Network, Quit); left/double-click opens the window.
   - Kill `explorer.exe` and restart it: icon comes back.
   - Quit from the tray exits both `Deskdrop.exe` and `Deskdrop.Tray.exe`.
   - Kill `Deskdrop.Tray.exe`: the app's watchdog restarts it within ~20 s.

## Next: phase 3 — .NET 10 + Native AOT

Target: installed ~70–90 MB, download ~30–40 MB (estimates). .NET 8 support ends November 2026, so the retarget is needed regardless.

1. **Retarget** `Deskdrop.WinUI.csproj` to `net10.0-windows10.0.26100.0`. Update package versions if the Windows App SDK needs it. Build and run once without AOT to separate retarget breaks from AOT breaks.
2. **Turn on AOT:** `<PublishAot>true</PublishAot>`; remove `PublishReadyToRun`, `PublishTrimmed=False`, `SuppressTrimAnalysis` and the `NoWarn` for `IL2026;IL2037;IL2057` — those suppressions would hide exactly the warnings that matter.
3. **Fix every IL/AOT warning. Rule: zero warnings.** An AOT warning is a runtime crash later. Known risk surface:
   - `WindowsIpcClient.cs`: `Send(object request)` serialises anonymous objects (`new { cmd = "rescan_peers" }`) with reflection — this breaks under AOT. Replace with typed request records plus a `JsonSerializerContext` (source generator), or build `JsonObject`s explicitly. Responses come back as `JsonDocument`, which is AOT-safe.
   - Other reflection/serialisation users: `DeskdropStore.cs`, `Services/DevicePicker.cs`, `Services/LocalSettingsStore.cs`, `Views/RemoteExplorerView.xaml.cs`.
   - XAML `{Binding}` → `{x:Bind}` wherever it appears.
   - P/Invoke: `[DllImport]` in `Native/NativeCore.cs` (39, the `deskdrop_core.dll` API), `GlobalHotKeyManager.cs` (6), `DashboardWindow.xaml.cs` (5), `App.xaml.cs` (4), `Services/GlobalDragMonitor.cs` (3), `Services/SystemTelemetryPoller.cs`, `Services/WindowIconHelper.cs`, `UI/EdgeDropWindow.xaml.cs` (1 each). Move to `[LibraryImport]` (source-generated marshalling); watch string and callback marshalling in `NativeCore.cs`.
   - Check `QRCoder` and `System.Drawing.Common` for AOT compatibility; replace `System.Drawing` use if it warns.
4. **Measure** with `scripts/windows-size.ps1`; expect `Microsoft.Windows.SDK.NET.dll` and most of the .NET runtime files to disappear.
5. **Test every screen and flow**, not just startup: pairing (PIN and QR), send/receive files, clipboard sync, notifications/toasts, remote explorer, settings save/load, quick access, command palette, tray.
6. Update `release.yml` (it uses `--self-contained true`; AOT publish needs the matching SDK on the runner) and `CHANGELOG.md`.

Rollback: phase 3 is one project file plus code fixes; revert its commits to return to the phase 2 build.

## Later: phase 4 (optional) — MSIX

MSIX package using Windows' shared Windows App Runtime (~20–30 MB), distributed through the Microsoft Store or winget (which also handles signing). Keep the self-contained per-user MSI as the fallback for locked-down PCs. Things to handle: `StartupTask` for launch at login, the LAN listener firewall rule in the manifest, the tray exe and `deskdrop_core.dll` declared in the package, and AppData/registry virtualisation for settings and the trust store. Decide after phase 3.

## Related notes

- Commit as `Chinmay Kudalkar <chinmaykudalkarrr@gmail.com>`, never a work email, and without AI co-author trailers. Check `git config user.email` in each repo on the new machine.
- Other uncommitted-then-pushed work from the same session (Android `full`/`play` flavors, `.deskdrop-part` downloads, macOS clipboard images, Windows Explorer-copy fix) is also untested; see `CHANGELOG.md` and `git log`.
