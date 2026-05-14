// CommandPaletteView.swift — ClipRelay macOS v4
// ⌘K palette: grouped commands, recent history, live sync-state labels.

import SwiftUI

// MARK: - Root

struct CommandPaletteView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var query         = ""
    @State private var selectedIndex = 0
    @State private var recentIDs:    [UUID] = []   // tracks recently run commands
    @FocusState private var inputFocused: Bool

    // ── Command catalogue ─────────────────────────────────────────────────────

    private var catalogue: [PaletteGroup] {
        let isSyncPaused = store.settings?.syncEnabled == false
        return [
            PaletteGroup(title: "Navigate", commands: [
                cmd(id: "nav.timeline", icon: "clock.arrow.circlepath",
                    label: "Timeline", hint: "View recent clipboard activity",
                    tint: CRTheme.brandElectric, shortcut: "⌘1")
                { store.selectedSection = .timeline },

                cmd(id: "nav.devices", icon: "rectangle.connected.to.line.below",
                    label: "Devices", hint: "Manage connected peers",
                    tint: CRTheme.accentGreen, shortcut: "⌘2")
                { store.selectedSection = .devices },

                cmd(id: "nav.trust", icon: "shield.checkered",
                    label: "Trust", hint: "Review device trust requests",
                    tint: CRTheme.accentOrange, shortcut: "⌘3")
                { store.selectedSection = .trust },

                cmd(id: "nav.settings", icon: "slider.horizontal.3",
                    label: "Settings", hint: "Tune sync and network options",
                    tint: CRTheme.accentPurple, shortcut: "⌘4")
                { store.selectedSection = .settings },
            ]),

            PaletteGroup(title: "Clipboard", commands: [
                cmd(id: "clip.send_all", icon: "paperplane.fill",
                    label: "Send Clipboard to All",
                    hint: "Push current clipboard to every connected peer",
                    tint: CRTheme.accentBlue)
                { store.sendCurrentClipboard(to: nil) },

                cmd(id: "clip.history", icon: "doc.on.clipboard",
                    label: "Open History Panel",
                    hint: "Browse and copy from clipboard history",
                    tint: CRTheme.brandElectric)
                { store.openHistoryPanel() },
            ]),

            PaletteGroup(title: "Network", commands: [
                cmd(id: "net.connect", icon: "network",
                    label: "Manual Connect…",
                    hint: "Connect to a peer by IP address and port",
                    tint: CRTheme.accentMint)
                { store.selectedSection = .devices },

                cmd(id: "net.sync_toggle",
                    icon: isSyncPaused ? "play.circle.fill" : "pause.circle.fill",
                    label: isSyncPaused ? "Resume Sync" : "Pause Sync",
                    hint: isSyncPaused
                        ? "Resume clipboard sync across all devices"
                        : "Pause clipboard sync across all devices",
                    tint: isSyncPaused ? CRTheme.accentGreen : CRTheme.accentOrange)
                { store.toggleSync() },

                cmd(id: "net.scan", icon: "antenna.radiowaves.left.and.right",
                    label: "Scan for Devices",
                    hint: "Search the local network for ClipRelay peers",
                    tint: CRTheme.accentTeal)
                { store.scanForDevices() },
            ]),
        ]
    }

    // ── Filtering ─────────────────────────────────────────────────────────────

    private var filtered: [PaletteGroup] {
        guard !query.isEmpty else { return catalogue }
        let q = query.lowercased()
        return catalogue.compactMap { group in
            let matched = group.commands.filter {
                $0.label.lowercased().contains(q) || $0.hint.lowercased().contains(q)
            }
            return matched.isEmpty ? nil : PaletteGroup(title: group.title, commands: matched)
        }
    }

    private var flat: [PaletteCommand] { filtered.flatMap { $0.commands } }

    // ── Body ──────────────────────────────────────────────────────────────────

    var body: some View {
        ZStack {
            CRHUDMaterial().ignoresSafeArea()
            LinearGradient(
                stops: [
                    .init(color: Color(red: 0.06, green: 0.08, blue: 0.16, opacity: 0.60), location: 0),
                    .init(color: Color(red: 0.03, green: 0.04, blue: 0.10, opacity: 0.74), location: 1)
                ],
                startPoint: .topLeading, endPoint: .bottomTrailing
            ).ignoresSafeArea()

            VStack(spacing: 0) {
                PaletteInputBar(query: $query, focused: $inputFocused, onSubmit: runSelected)

                Rectangle().fill(Color(white: 1, opacity: 0.07)).frame(height: 0.5)

                if flat.isEmpty {
                    PaletteEmptyState()
                } else {
                    ScrollView(.vertical, showsIndicators: false) {
                        // Swift doesn't allow mutable state inside body.
                        // Use a helper to compute flat offsets.
                        PaletteResultList(
                            groups:        filtered,
                            flat:          flat,
                            selectedIndex: selectedIndex,
                            showHeaders:   query.isEmpty,
                            recentIDs:     recentIDs,
                            onSelect:      { idx in selectedIndex = idx },
                            onRun:         { idx in selectedIndex = idx; runSelected() }
                        )
                    }
                }

                Rectangle().fill(Color(white: 1, opacity: 0.07)).frame(height: 0.5)
                PaletteFooter()
            }
        }
        .frame(width: 520)
        .fixedSize(horizontal: false, vertical: true)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .strokeBorder(Color(white: 1, opacity: 0.08), lineWidth: 0.5)
        }
        .shadow(color: .black.opacity(0.56), radius: 70, x: 0, y: 28)
        .environment(\.colorScheme, .dark)
        .onAppear { inputFocused = true; selectedIndex = 0 }
        .onChange(of: query) { _ in selectedIndex = 0 }
        .background(
            Group {
                Button("") { navigate(-1) }.keyboardShortcut(.upArrow,   modifiers: [])
                Button("") { navigate(+1) }.keyboardShortcut(.downArrow, modifiers: [])
                Button("") { runSelected() }.keyboardShortcut(.return,   modifiers: [])
            }
            .frame(width: 0, height: 0).opacity(0)
        )
    }

    // MARK: Helpers

    private func navigate(_ delta: Int) {
        let count = flat.count; guard count > 0 else { return }
        selectedIndex = min(count - 1, max(0, selectedIndex + delta))
    }

    private func runSelected() {
        guard selectedIndex < flat.count else { return }
        let chosen = flat[selectedIndex]
        recentIDs = ([chosen.id] + recentIDs).prefix(5).map { $0 }
        chosen.action()
    }

    private func cmd(id: String, icon: String, label: String, hint: String,
                     tint: Color, shortcut: String? = nil,
                     action: @escaping () -> Void) -> PaletteCommand {
        PaletteCommand(id: id, icon: icon, label: label, hint: hint,
                       tint: tint, shortcut: shortcut, action: action)
    }
}

// MARK: - Result List (extracted to avoid mutable-in-body issue)

private struct PaletteResultList: View {
    let groups:        [PaletteGroup]
    let flat:          [PaletteCommand]
    let selectedIndex: Int
    let showHeaders:   Bool
    let recentIDs:     [UUID]
    let onSelect:      (Int) -> Void
    let onRun:         (Int) -> Void

    var body: some View {
        VStack(spacing: 0) {
            var runningIdx = 0
            ForEach(groups) { group in
                if showHeaders {
                    PaletteGroupHeader(title: group.title)
                }
                ForEach(group.commands) { cmd in
                    let myIdx = runningIdx
                    let isRecent = recentIDs.contains(cmd.id)
                    PaletteRow(command: cmd, isSelected: myIdx == selectedIndex, isRecent: isRecent)
                        .onTapGesture { onRun(myIdx) }
                        .onHover { if $0 { onSelect(myIdx) } }
                    let _ = { runningIdx += 1 }()
                }
            }
        }
        .padding(.horizontal, 8).padding(.vertical, 6)
    }
}

// MARK: - Input Bar

private struct PaletteInputBar: View {
    @Binding var query: String
    var focused: FocusState<Bool>.Binding
    let onSubmit: () -> Void

    var body: some View {
        HStack(spacing: 11) {
            CRAppIconMark(size: 26)

            TextField("Type a command…", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: 15.5))
                .foregroundStyle(.white)
                .focused(focused)
                .onSubmit { onSubmit() }

            if !query.isEmpty {
                Button { withAnimation(.crFast) { query = "" } } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 14.5)).foregroundStyle(Color(white: 1, opacity: 0.28))
                }
                .buttonStyle(.plain).transition(.scale(scale: 0.75).combined(with: .opacity))
            } else {
                KbdChip("⌘K")
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .animation(.crFast, value: query.isEmpty)
    }
}

// MARK: - Group Header

private struct PaletteGroupHeader: View {
    let title: String
    var body: some View {
        HStack {
            Text(title.uppercased())
                .font(.system(size: 9.5, weight: .bold)).tracking(1.1)
                .foregroundStyle(Color(white: 1, opacity: 0.22))
            Spacer()
        }
        .padding(.horizontal, 10).padding(.top, 10).padding(.bottom, 3)
    }
}

// MARK: - Palette Row

private struct PaletteRow: View {
    let command:    PaletteCommand
    var isSelected: Bool
    var isRecent:   Bool

    var body: some View {
        HStack(spacing: 11) {
            ZStack {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(command.tint.opacity(isSelected ? 0.22 : 0.10))
                    .frame(width: 30, height: 30)
                Image(systemName: command.icon)
                    .font(.system(size: 12.5, weight: .semibold))
                    .foregroundStyle(command.tint).symbolRenderingMode(.hierarchical)
            }

            VStack(alignment: .leading, spacing: 1.5) {
                HStack(spacing: 6) {
                    Text(command.label)
                        .font(.system(size: 13, weight: isSelected ? .semibold : .medium))
                        .foregroundStyle(.white.opacity(isSelected ? 1.0 : 0.78))
                    if isRecent {
                        CRTag(text: "recent", tint: CRTheme.brandElectric)
                    }
                }
                Text(command.hint)
                    .font(.system(size: 11))
                    .foregroundStyle(.white.opacity(isSelected ? 0.44 : 0.28))
            }

            Spacer(minLength: 0)

            HStack(spacing: 5) {
                if let sc = command.shortcut {
                    Text(sc)
                        .font(.system(size: 10, weight: .semibold, design: .rounded))
                        .foregroundStyle(Color(white: 1, opacity: isSelected ? 0.38 : 0.18))
                        .padding(.horizontal, 6).padding(.vertical, 2.5)
                        .background {
                            RoundedRectangle(cornerRadius: 5, style: .continuous)
                                .fill(Color(white: 1, opacity: 0.07))
                                .overlay {
                                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                                        .strokeBorder(Color(white: 1, opacity: 0.09), lineWidth: 0.5)
                                }
                        }
                }
                if isSelected { KbdChip("↵").transition(.opacity) }
            }
        }
        .padding(.horizontal, 10).padding(.vertical, 8)
        .background {
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(isSelected ? Color(white: 1, opacity: 0.09) : .clear)
        }
        .contentShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        .animation(.crFast, value: isSelected)
    }
}

// MARK: - Empty State

private struct PaletteEmptyState: View {
    var body: some View {
        VStack(spacing: 11) {
            Image(systemName: "questionmark.circle")
                .font(.system(size: 26, weight: .ultraLight))
                .foregroundStyle(Color(white: 1, opacity: 0.16)).symbolRenderingMode(.hierarchical)
            Text("No matching commands")
                .font(.system(size: 13)).foregroundStyle(Color(white: 1, opacity: 0.28))
        }
        .frame(maxWidth: .infinity).padding(.vertical, 30)
    }
}

// MARK: - Footer

private struct PaletteFooter: View {
    var body: some View {
        HStack(spacing: 16) {
            PaletteHint(keys: ["↑", "↓"], label: "select")
            PaletteHint(keys: ["↵"],       label: "run")
            PaletteHint(keys: ["Esc"],      label: "close")
            Spacer()
        }
        .padding(.horizontal, 16).padding(.vertical, 10)
    }
}

private struct PaletteHint: View {
    let keys: [String]; let label: String
    var body: some View {
        HStack(spacing: 3) {
            ForEach(keys, id: \.self) { KbdChip($0) }
            Text(label).font(.system(size: 10.5)).foregroundStyle(Color(white: 1, opacity: 0.20))
        }
    }
}

// MARK: - Data Models

private struct PaletteGroup: Identifiable {
    let id    = UUID()
    let title: String
    let commands: [PaletteCommand]
}

struct PaletteCommand: Identifiable {
    let id:       UUID
    let icon:     String
    let label:    String
    let hint:     String
    var tint:     Color   = CRTheme.brandElectric
    var shortcut: String? = nil
    let action:   () -> Void

    init(id: String, icon: String, label: String, hint: String,
         tint: Color, shortcut: String? = nil, action: @escaping () -> Void) {
        self.id       = UUID(uuidString: id) ?? UUID()
        self.icon     = icon; self.label = label; self.hint = hint
        self.tint     = tint; self.shortcut = shortcut; self.action = action
    }
}

// MARK: - Accent alias (local)

private extension CRTheme {
    static let accentMint = Color(hex: 0x00C7BE)
    static let accentTeal = Color(hex: 0x32ADE6)
}
