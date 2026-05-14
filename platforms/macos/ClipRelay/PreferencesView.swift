// PreferencesView.swift — ClipRelay macOS v4
// System Settings aesthetic: icon tabs, live dirty state, port validation, fingerprint display.

import SwiftUI

// MARK: - Root

struct PreferencesView: View {
    @ObservedObject var store: ClipRelayStore

    @State private var tab              = SettingsTab.general
    @State private var copy             = ClipRelaySettingsSnapshot.defaults
    @State private var patternDraft     = ""
    @State private var isDirty          = false
    @State private var portString       = "47823"
    @State private var portIsInvalid    = false

    var body: some View {
        VStack(spacing: 0) {
            PrefsToolbar(tab: $tab)
            CRDivider()

            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    pane
                        .padding(.horizontal, 24)
                        .padding(.top, 20)
                        .padding(.bottom, 28)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            // Mark dirty on any meaningful change
            .onChange(of: copy.deviceName)              { _ in isDirty = true }
            .onChange(of: copy.syncEnabled)             { _ in isDirty = true }
            .onChange(of: copy.startOnLogin)            { _ in isDirty = true }
            .onChange(of: copy.blockSensitiveText)      { _ in isDirty = true }
            .onChange(of: copy.requireTofuConfirmation) { _ in isDirty = true }
            .onChange(of: copy.historyLimit)            { _ in isDirty = true }
            .onChange(of: copy.maxPayloadBytes)         { _ in isDirty = true }
            .onChange(of: copy.ignorePatterns)          { _ in isDirty = true }

            PrefsFooter(
                isDirty:      isDirty,
                portIsInvalid: portIsInvalid,
                onRevert: {
                    if let s = store.settings { copy = s; portString = "\(s.port)" }
                    portIsInvalid = false
                    isDirty       = false
                },
                onSave: {
                    guard !portIsInvalid else { return }
                    store.saveSettings(copy)
                    isDirty = false
                }
            )
        }
        .frame(minWidth: 560, minHeight: 580)
        .background(CRTheme.surfaceElevated.ignoresSafeArea())
        .onAppear {
            if let s = store.settings { copy = s; portString = "\(s.port)" }
        }
    }

    @ViewBuilder private var pane: some View {
        switch tab {
        case .general:  GeneralPane(copy: $copy)
        case .sync:     SyncPane(copy: $copy, patternDraft: $patternDraft)
        case .network:  NetworkPane(copy: $copy, portString: $portString, portIsInvalid: $portIsInvalid)
                            .onChange(of: portString) { v in
                                if let p = UInt16(v), p > 1024 { copy.port = p; portIsInvalid = false; isDirty = true }
                                else { portIsInvalid = true }
                            }
        case .security: SecurityPane(copy: $copy, store: store)
        }
    }
}

// MARK: - Tab Bar

private enum SettingsTab: String, CaseIterable, Identifiable {
    case general, sync, network, security
    var id: String { rawValue }
    var label: String { rawValue.capitalized }
    var icon:  String {
        switch self {
        case .general:  return "person.crop.circle"
        case .sync:     return "arrow.triangle.2.circlepath"
        case .network:  return "network"
        case .security: return "lock.shield"
        }
    }
    var tint: Color {
        switch self {
        case .general:  return CRTheme.accentBlue
        case .sync:     return CRTheme.accentGreen
        case .network:  return CRTheme.accentIndigo
        case .security: return CRTheme.accentOrange
        }
    }
}

private struct PrefsToolbar: View {
    @Binding var tab: SettingsTab
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Settings").font(.system(size: 19, weight: .bold)).foregroundStyle(CRTheme.ink)
                    Text("Configure ClipRelay for your workflow")
                        .font(.system(size: 12)).foregroundStyle(CRTheme.inkSoft)
                }
                Spacer()
            }
            .padding(.horizontal, 24).padding(.top, 20).padding(.bottom, 14)

            HStack(spacing: 2) {
                ForEach(SettingsTab.allCases) { t in PrefsTabChip(tab: t, isSelected: tab == t) { tab = t } }
                Spacer()
            }
            .padding(.horizontal, 20).padding(.bottom, 12)
        }
    }
}

private struct PrefsTabChip: View {
    let tab: SettingsTab; let isSelected: Bool; let action: () -> Void
    @State private var hovered = false
    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: isSelected ? tab.icon + ".fill" : tab.icon)
                    .font(.system(size: 12.5, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(isSelected ? tab.tint : CRTheme.inkSoft).symbolRenderingMode(.hierarchical)
                Text(tab.label)
                    .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(isSelected ? tab.tint : CRTheme.inkSoft)
            }
            .padding(.horizontal, 13).padding(.vertical, 6.5)
            .background {
                Capsule()
                    .fill(isSelected ? tab.tint.opacity(0.09) : (hovered ? CRTheme.surface : .clear))
                    .overlay {
                        if isSelected { Capsule().strokeBorder(tab.tint.opacity(0.22), lineWidth: 0.5) }
                    }
            }
        }
        .buttonStyle(.plain).onHover { hovered = $0 }
        .animation(.crFast, value: isSelected).animation(.crFast, value: hovered)
    }
}

// MARK: - Footer

private struct PrefsFooter: View {
    let isDirty:       Bool
    let portIsInvalid: Bool
    let onRevert:      () -> Void
    let onSave:        () -> Void

    var body: some View {
        VStack(spacing: 0) {
            CRDivider()
            HStack(spacing: 10) {
                // Status indicators
                Group {
                    if portIsInvalid {
                        HStack(spacing: 5) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .font(.system(size: 11)).foregroundStyle(CRTheme.accentRed)
                            Text("Port must be 1025–65535")
                                .font(.system(size: 12, weight: .medium)).foregroundStyle(CRTheme.accentRed)
                        }
                        .transition(.move(edge: .leading).combined(with: .opacity))
                    } else if isDirty {
                        HStack(spacing: 5) {
                            Circle().fill(CRTheme.accentYellow).frame(width: 5.5, height: 5.5)
                            Text("Unsaved changes")
                                .font(.system(size: 12, weight: .medium)).foregroundStyle(CRTheme.accentYellow)
                        }
                        .transition(.move(edge: .leading).combined(with: .opacity))
                    }
                }
                Spacer()
                Button("Revert", action: onRevert).buttonStyle(CRSecondaryButtonStyle()).disabled(!isDirty)
                Button("Save Changes", action: onSave)
                    .buttonStyle(CRPrimaryButtonStyle())
                    .disabled(!isDirty || portIsInvalid)
            }
            .padding(.horizontal, 24).padding(.vertical, 12)
        }
        .animation(.crFast, value: isDirty)
        .animation(.crFast, value: portIsInvalid)
    }
}

// MARK: - General Pane

private struct GeneralPane: View {
    @Binding var copy: ClipRelaySettingsSnapshot

    var body: some View {
        PrefsSection(title: "Identity", icon: "person.crop.circle.fill", tint: CRTheme.accentBlue) {
            PrefsRow(icon: "tag.fill", label: "Device name",
                     description: "How this Mac appears to other ClipRelay peers.") {
                TextField("MacBook Pro", text: $copy.deviceName).crInput().frame(maxWidth: 220)
            }
            PrefsDivider()
            PrefsRow(icon: "power", label: "Start on login",
                     description: "Launch ClipRelay automatically at login.") {
                Toggle("", isOn: $copy.startOnLogin).labelsHidden()
            }
        }

        PrefsSection(title: "Notifications", icon: "bell.badge.fill", tint: CRTheme.accentPurple) {
            PrefsRow(icon: "bell.fill", label: "Show receive notification",
                     description: "Display a banner when a peer pushes content to this Mac.") {
                Toggle("", isOn: $copy.showReceiveNotification).labelsHidden()
            }
        }

        PrefsSection(title: "History", icon: "clock.fill", tint: CRTheme.accentIndigo) {
            PrefsRow(icon: "list.number", label: "Max timeline items",
                     description: "Oldest entries are dropped when the limit is reached.") {
                Stepper("\(copy.historyLimit) items",
                        value: $copy.historyLimit, in: 10...500, step: 10)
                    .frame(maxWidth: 190)
            }
            PrefsDivider()
            PrefsRow(icon: "doc.text", label: "Max text size per item",
                     description: "Clipboard text larger than this won't be stored locally.") {
                Stepper("\(copy.maxHistoryTextBytes / 1024) KB",
                        value: Binding(
                            get: { copy.maxHistoryTextBytes / 1024 },
                            set: { copy.maxHistoryTextBytes = $0 * 1024 }
                        ), in: 4...512, step: 4)
                    .frame(maxWidth: 190)
            }
        }
    }
}

// MARK: - Sync Pane

private struct SyncPane: View {
    @Binding var copy: ClipRelaySettingsSnapshot
    @Binding var patternDraft: String

    var body: some View {
        PrefsSection(title: "Sync Control", icon: "arrow.triangle.2.circlepath", tint: CRTheme.accentGreen) {
            PrefsRow(icon: "power.circle.fill", label: "Enable sync",
                     description: "Toggle clipboard sync for all devices at once.") {
                Toggle("", isOn: $copy.syncEnabled).labelsHidden()
            }
            PrefsDivider()
            PrefsRow(icon: "gearshape.2.fill", label: "Sync mode",
                     description: "Auto syncs immediately. Manual requires an explicit trigger.") {
                Picker("", selection: $copy.syncMode) {
                    Text("Auto").tag(SyncModeModel.auto)
                    Text("Manual").tag(SyncModeModel.manual)
                    Text("Receive only").tag(SyncModeModel.receive)
                }
                .pickerStyle(.segmented).labelsHidden().frame(maxWidth: 220)
            }
        }

        PrefsSection(title: "Content Types", icon: "doc.on.doc.fill", tint: CRTheme.accentBlue) {
            PrefsRow(icon: "doc.text.fill",  label: "Text")   { Toggle("", isOn: $copy.syncText).labelsHidden() }
            PrefsDivider()
            PrefsRow(icon: "photo.fill",     label: "Images") { Toggle("", isOn: $copy.syncImages).labelsHidden() }
            PrefsDivider()
            PrefsRow(icon: "folder.fill",    label: "Files")  { Toggle("", isOn: $copy.syncFiles).labelsHidden() }
            PrefsDivider()
            PrefsRow(icon: "arrow.up.arrow.down.circle.fill", label: "Max payload",
                     description: "Largest item that will be synced to peers.") {
                Stepper("\(Int(copy.maxPayloadBytes / 1024 / 1024)) MB",
                        value: Binding(
                            get: { Int(copy.maxPayloadBytes / 1024 / 1024) },
                            set: { copy.maxPayloadBytes = UInt64($0) * 1024 * 1024 }
                        ), in: 1...512)
                    .frame(maxWidth: 180)
            }
        }

        PrefsSection(title: "Ignore Patterns", icon: "eye.slash.fill", tint: CRTheme.accentRed) {
            VStack(alignment: .leading, spacing: 11) {
                Text("Items matching these substrings won't be synced. Useful for passwords, OTPs, and API keys.")
                    .font(.system(size: 12)).foregroundStyle(CRTheme.inkSoft).lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)

                HStack(spacing: 8) {
                    TextField("e.g. password, Bearer, sk-", text: $patternDraft)
                        .crInput()
                    Button("Add") {
                        let t = patternDraft.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !t.isEmpty else { return }
                        copy.ignorePatterns.append(t)
                        patternDraft = ""
                    }
                    .buttonStyle(CRPrimaryButtonStyle())
                    .disabled(patternDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }

                if !copy.ignorePatterns.isEmpty {
                    VStack(spacing: 3) {
                        ForEach(copy.ignorePatterns, id: \.self) { pattern in
                            HStack(spacing: 9) {
                                Button {
                                    copy.ignorePatterns.removeAll { $0 == pattern }
                                } label: {
                                    Image(systemName: "xmark.circle.fill")
                                        .font(.system(size: 14))
                                        .foregroundStyle(CRTheme.accentRed.opacity(0.60))
                                }
                                .buttonStyle(.plain)
                                Text(pattern)
                                    .font(.system(size: 12, design: .monospaced)).foregroundStyle(CRTheme.ink)
                                Spacer()
                                CRTag(text: "substring", tint: CRTheme.inkSubtle)
                            }
                            .padding(.horizontal, 10).padding(.vertical, 7)
                            .background {
                                RoundedRectangle(cornerRadius: 7, style: .continuous)
                                    .fill(CRTheme.surface)
                                    .overlay {
                                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                                            .strokeBorder(CRTheme.stroke.opacity(0.40), lineWidth: 0.5)
                                    }
                            }
                        }
                    }
                }
            }
            .padding(.horizontal, 14).padding(.vertical, 13)
        }
    }
}

// MARK: - Network Pane

private struct NetworkPane: View {
    @Binding var copy: ClipRelaySettingsSnapshot
    @Binding var portString: String
    @Binding var portIsInvalid: Bool

    var body: some View {
        PrefsSection(title: "Listener", icon: "antenna.radiowaves.left.and.right", tint: CRTheme.accentIndigo) {
            PrefsRow(icon: "number.circle.fill", label: "Port",
                     description: "TCP port the daemon binds to. Changes take effect on restart.") {
                TextField("47823", text: $portString)
                    .crInput(invalid: portIsInvalid)
                    .frame(width: 90)
            }
        }

        PrefsSection(title: "Clipboard Polling", icon: "timer", tint: CRTheme.accentBlue) {
            PrefsRow(icon: "clock.arrow.circlepath", label: "Poll interval",
                     description: "How often the daemon checks for new clipboard content.") {
                Stepper("\(copy.clipboardPollMs) ms",
                        value: Binding(
                            get: { Int(copy.clipboardPollMs) },
                            set: { copy.clipboardPollMs = UInt64($0) }
                        ), in: 50...2000, step: 25)
                    .frame(maxWidth: 190)
            }
        }

        PrefsSection(title: "Smart Sync", icon: "sparkles", tint: CRTheme.accentGreen) {
            PrefsRow(icon: "rectangle.2.swap", label: "Duplicate window",
                     description: "Content seen within this window won't be re-broadcast.") {
                Stepper("\(copy.smartSyncDuplicateWindowMs) ms",
                        value: Binding(
                            get: { Int(copy.smartSyncDuplicateWindowMs) },
                            set: { copy.smartSyncDuplicateWindowMs = UInt64($0) }
                        ), in: 100...10000, step: 100)
                    .frame(maxWidth: 205)
            }
            PrefsDivider()
            PrefsRow(icon: "clock.badge.checkmark", label: "Debounce window",
                     description: "Delay after a copy event before broadcasting.") {
                Stepper("\(copy.smartSyncDebounceMs) ms",
                        value: Binding(
                            get: { Int(copy.smartSyncDebounceMs) },
                            set: { copy.smartSyncDebounceMs = UInt64($0) }
                        ), in: 25...2000, step: 25)
                    .frame(maxWidth: 205)
            }
        }

        PrefsSection(title: "Rate Limiting", icon: "gauge.with.dots.needle.67percent", tint: CRTheme.accentOrange) {
            PrefsRow(icon: "arrow.up.circle.fill", label: "Max pushes / sec") {
                Stepper("\(copy.maxPushesPerSec)", value: $copy.maxPushesPerSec, in: 1...60)
                    .frame(maxWidth: 165)
            }
            PrefsDivider()
            PrefsRow(icon: "bolt.circle.fill", label: "Burst allowance",
                     description: "Extra pushes allowed before the sustained limit applies.") {
                Stepper("\(copy.rateLimitBurst)", value: $copy.rateLimitBurst, in: 1...20)
                    .frame(maxWidth: 165)
            }
        }
    }
}

// MARK: - Security Pane

private struct SecurityPane: View {
    @Binding var copy: ClipRelaySettingsSnapshot
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        PrefsSection(title: "Trust", icon: "shield.checkered", tint: CRTheme.accentGreen) {
            PrefsRow(icon: "person.badge.clock.fill", label: "Require trust confirmation",
                     description: "Show a prompt for every new device before allowing sync (TOFU model).") {
                Toggle("", isOn: $copy.requireTofuConfirmation).labelsHidden()
            }
        }

        PrefsSection(title: "Content Filtering", icon: "eye.slash.circle.fill", tint: CRTheme.accentOrange) {
            PrefsRow(icon: "key.slash.fill", label: "Block likely secrets",
                     description: "Skip sync for content matching common secret patterns.") {
                Toggle("", isOn: $copy.blockSensitiveText).labelsHidden()
            }
        }

        // This device's fingerprint
        if let deviceName = store.settings?.deviceName, !deviceName.isEmpty {
            PrefsSection(title: "This Device", icon: "checkmark.seal.fill", tint: CRTheme.accentBlue) {
                PrefsRow(icon: "person.crop.circle.fill", label: "Name") {
                    Text(deviceName)
                        .font(.system(size: 13)).foregroundStyle(CRTheme.inkSoft)
                }
            }
        }

        // Security model info card
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                ZStack {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(CRTheme.accentGreen.opacity(0.10)).frame(width: 22, height: 22)
                    Image(systemName: "checkmark.shield.fill")
                        .font(.system(size: 10.5, weight: .semibold)).foregroundStyle(CRTheme.accentGreen)
                }
                Text("Security Architecture")
                    .font(.system(size: 13, weight: .semibold)).foregroundStyle(CRTheme.ink)
            }

            VStack(alignment: .leading, spacing: 8) {
                SecInfoRow(icon: "lock.fill",        tint: CRTheme.accentBlue,
                           text: "Traffic is encrypted end-to-end with the Noise protocol.")
                SecInfoRow(icon: "person.2.slash",   tint: CRTheme.accentGreen,
                           text: "No relay servers. Peers connect directly on your local network.")
                SecInfoRow(icon: "checkmark.shield", tint: CRTheme.accentIndigo,
                           text: "TOFU trust model — each device fingerprint is verified on first connection.")
            }
        }
        .padding(14)
        .background {
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .fill(CRTheme.surface)
                .overlay {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .strokeBorder(CRTheme.stroke.opacity(0.40), lineWidth: 0.5)
                }
        }
        .padding(.bottom, 24)
    }
}

private struct SecInfoRow: View {
    let icon: String; let tint: Color; let text: String
    var body: some View {
        HStack(alignment: .top, spacing: 9) {
            ZStack {
                Circle().fill(tint.opacity(0.09)).frame(width: 22, height: 22)
                Image(systemName: icon).font(.system(size: 10, weight: .semibold)).foregroundStyle(tint)
            }
            Text(text).font(.system(size: 12)).foregroundStyle(CRTheme.inkSoft)
                .fixedSize(horizontal: false, vertical: true).lineSpacing(1.5)
            Spacer()
        }
    }
}

// MARK: - Shared Section + Row + Divider

private struct PrefsSection<Content: View>: View {
    let title: String; let icon: String; let tint: Color
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 6) {
                ZStack {
                    RoundedRectangle(cornerRadius: 5.5, style: .continuous)
                        .fill(tint.opacity(0.10)).frame(width: 20, height: 20)
                    Image(systemName: icon).font(.system(size: 9.5, weight: .semibold))
                        .foregroundStyle(tint).symbolRenderingMode(.hierarchical)
                }
                Text(title.uppercased())
                    .font(.system(size: 10, weight: .bold)).tracking(0.7).foregroundStyle(CRTheme.inkSubtle)
            }
            .padding(.leading, 2)

            VStack(spacing: 0) { content() }
                .background(CRTheme.surfaceStrong)
                .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .strokeBorder(CRTheme.stroke.opacity(0.40), lineWidth: 0.5)
                }
        }
        .padding(.bottom, 22)
    }
}

private struct PrefsRow<Control: View>: View {
    var icon: String? = nil
    let label: String
    var description: String? = nil
    @ViewBuilder var control: () -> Control

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            HStack(alignment: .center, spacing: 9) {
                if let icon {
                    Image(systemName: icon).font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(CRTheme.inkSubtle).frame(width: 18)
                        .symbolRenderingMode(.hierarchical)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(label).font(.system(size: 13, weight: .medium)).foregroundStyle(CRTheme.ink)
                    if let d = description {
                        Text(d).font(.system(size: 11.5)).foregroundStyle(CRTheme.inkSoft)
                            .fixedSize(horizontal: false, vertical: true).lineSpacing(1)
                    }
                }
            }
            Spacer()
            control()
        }
        .padding(.horizontal, 14).padding(.vertical, 11)
    }
}

private struct PrefsDivider: View {
    var body: some View { Divider().padding(.leading, 44) }
}

// MARK: - Settings Snapshot default

extension ClipRelaySettingsSnapshot {
    static var defaults: ClipRelaySettingsSnapshot {
        ClipRelaySettingsSnapshot(
            port: 47823, deviceName: "", syncEnabled: true,
            syncText: true, syncImages: true, syncFiles: true,
            syncMode: .auto, maxPayloadBytes: 64 * 1024 * 1024,
            historyLimit: 50, maxHistoryTextBytes: 64 * 1024,
            showReceiveNotification: true, requireTofuConfirmation: true,
            blockedDeviceIds: [], blockSensitiveText: true,
            ignorePatterns: [], clipboardPollMs: 100,
            maxPushesPerSec: 10, rateLimitBurst: 3,
            smartSyncDuplicateWindowMs: 1500, smartSyncDebounceMs: 150,
            startOnLogin: false
        )
    }
}
