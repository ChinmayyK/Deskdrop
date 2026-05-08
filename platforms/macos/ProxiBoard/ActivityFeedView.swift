// ActivityFeedView.swift
// ClipRelay — macOS Timeline-First Clipboard & Activity Feed
//
// Shows all cross-device events in reverse-chronological order.
// Remote clipboard items display an "Apply" button instead of auto-overwriting.
// File transfers show progress bars and Accept/Reject actions.

import SwiftUI

struct ActivityFeedView: View {
    @EnvironmentObject var store: ClipRelayStore
    @State private var searchText = ""

    private var filteredEntries: [IpcActivityEntry] {
        if searchText.isEmpty { return store.activityFeed }
        return store.activityFeed.filter {
            $0.summary.localizedCaseInsensitiveContains(searchText) ||
            $0.device_name.localizedCaseInsensitiveContains(searchText) ||
            ($0.text_preview?.localizedCaseInsensitiveContains(searchText) ?? false)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // ── Search bar ────────────────────────────────────────────────────
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                    .font(.caption)
                TextField("Filter activity…", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.callout)
                if !searchText.isEmpty {
                    Button { searchText = "" } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(NSColor.controlBackgroundColor))

            Divider()

            // ── Pending clipboard items banner ────────────────────────────────
            let pending = store.activityFeed.filter { $0.isApplicable }
            if !pending.isEmpty && searchText.isEmpty {
                PendingClipboardBanner(items: pending)
                Divider()
            }

            // ── Active file transfers ─────────────────────────────────────────
            if !store.activeTransfers.isEmpty && searchText.isEmpty {
                ForEach(store.activeTransfers) { transfer in
                    FileTransferRowView(transfer: transfer)
                    Divider()
                }
            }

            // ── Feed entries ──────────────────────────────────────────────────
            if filteredEntries.isEmpty {
                emptyState
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(filteredEntries) { entry in
                            ActivityEntryRowView(entry: entry)
                            Divider().padding(.leading, 40)
                        }
                    }
                }
            }
        }
        .frame(minWidth: 340, minHeight: 400)
        .task { await store.refreshActivityFeed() }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "clock.arrow.circlepath")
                .font(.largeTitle)
                .foregroundColor(.secondary)
            Text("No activity yet")
                .font(.subheadline)
            Text("Clipboard copies and file transfers across devices will appear here.")
                .font(.caption)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 24)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// ── Pending clipboard items banner ─────────────────────────────────────────────

private struct PendingClipboardBanner: View {
    let items: [IpcActivityEntry]
    @EnvironmentObject var store: ClipRelayStore

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Label("\(items.count) clipboard item\(items.count == 1 ? "" : "s") waiting",
                  systemImage: "doc.on.clipboard")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundColor(.accentColor)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(items) { item in
                        PendingClipboardChip(entry: item)
                    }
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(Color.accentColor.opacity(0.06))
    }
}

private struct PendingClipboardChip: View {
    let entry: IpcActivityEntry
    @EnvironmentObject var store: ClipRelayStore
    @State private var applying = false

    var body: some View {
        HStack(spacing: 6) {
            VStack(alignment: .leading, spacing: 1) {
                Text(entry.device_name)
                    .font(.caption2)
                    .foregroundColor(.secondary)
                Text(entry.text_preview ?? "Clipboard item")
                    .font(.caption)
                    .lineLimit(1)
            }
            Button {
                applying = true
                Task {
                    await store.applyClipboard(entry: entry)
                    applying = false
                }
            } label: {
                if applying {
                    ProgressView().controlSize(.mini)
                } else {
                    Text("Apply")
                        .font(.caption)
                        .fontWeight(.medium)
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.mini)
            .disabled(applying)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(Color(NSColor.controlBackgroundColor))
        .cornerRadius(6)
        .overlay(RoundedRectangle(cornerRadius: 6)
            .stroke(Color.accentColor.opacity(0.3), lineWidth: 1))
    }
}

// ── File transfer progress row ─────────────────────────────────────────────────

struct FileTransferRowView: View {
    let transfer: FileTransferState
    @EnvironmentObject var store: ClipRelayStore

    var body: some View {
        HStack(spacing: 10) {
            // Icon
            Image(systemName: transferIcon)
                .foregroundColor(transferColor)
                .font(.title3)
                .frame(width: 28)

            VStack(alignment: .leading, spacing: 3) {
                HStack {
                    Text(transfer.fileName)
                        .font(.callout)
                        .lineLimit(1)
                    Spacer()
                    Text(transfer.formattedSize)
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                HStack {
                    Text("From \(transfer.fromDeviceName)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Spacer()
                    statusText
                }
                if case .transferring = transfer.status {
                    ProgressView(value: Double(transfer.percent), total: 100)
                        .progressViewStyle(.linear)
                        .tint(.accentColor)
                }
            }

            // Actions
            actionButtons
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Color(NSColor.controlBackgroundColor).opacity(0.5))
    }

    @ViewBuilder
    private var actionButtons: some View {
        switch transfer.status {
        case .incoming:
            HStack(spacing: 6) {
                Button("Accept") { store.acceptFileTransfer(transfer) }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                Button("Reject") { store.rejectFileTransfer(transfer) }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
            }
        case .transferring:
            Button("Cancel") { store.cancelFileTransfer(transfer) }
                .buttonStyle(.bordered)
                .controlSize(.small)
        case .complete(let path):
            Button {
                NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "")
            } label: {
                Label("Show in Finder", systemImage: "folder")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
        default:
            EmptyView()
        }
    }

    private var statusText: some View {
        Group {
            switch transfer.status {
            case .incoming:      Text("Incoming").foregroundColor(.orange)
            case .transferring:  Text("\(transfer.percent)%").foregroundColor(.accentColor)
            case .verifying:     Text("Verifying…").foregroundColor(.secondary)
            case .complete:      Text("Complete").foregroundColor(.green)
            case .failed(let r): Text("Failed: \(r)").foregroundColor(.red)
            case .cancelled:     Text("Cancelled").foregroundColor(.secondary)
            }
        }
        .font(.caption2)
        .fontWeight(.medium)
    }

    private var transferIcon: String {
        switch transfer.status {
        case .incoming:      return "arrow.down.circle"
        case .transferring:  return "arrow.down.circle.fill"
        case .complete:      return "checkmark.circle.fill"
        case .failed:        return "xmark.circle.fill"
        case .cancelled:     return "minus.circle"
        default:             return "doc.circle"
        }
    }

    private var transferColor: Color {
        switch transfer.status {
        case .incoming:      return .orange
        case .transferring:  return .accentColor
        case .complete:      return .green
        case .failed:        return .red
        default:             return .secondary
        }
    }
}

// ── Single activity entry row ──────────────────────────────────────────────────

private struct ActivityEntryRowView: View {
    let entry: IpcActivityEntry
    @EnvironmentObject var store: ClipRelayStore
    @State private var applying = false
    @State private var expanded = false

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            // Kind icon
            Image(systemName: kindIcon)
                .foregroundColor(kindColor)
                .font(.callout)
                .frame(width: 20, alignment: .center)
                .padding(.top, 2)

            VStack(alignment: .leading, spacing: 2) {
                // Summary line
                Text(entry.summary)
                    .font(.callout)
                    .lineLimit(expanded ? nil : 2)

                // Relay path (mesh traceability)
                if !entry.relay_path.isEmpty {
                    Text(entry.relay_path.joined(separator: " → "))
                        .font(.caption2)
                        .foregroundColor(.secondary.opacity(0.7))
                }

                // Text preview for clipboard items
                if let preview = entry.text_preview, !preview.isEmpty, expanded {
                    Text(preview)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .padding(6)
                        .background(Color(NSColor.textBackgroundColor))
                        .cornerRadius(4)
                        .onTapGesture { expanded = false }
                }

                // Timestamp
                Text(formattedTime(entry.timestamp_ms))
                    .font(.caption2)
                    .foregroundColor(.secondary.opacity(0.6))
            }

            Spacer()

            // Apply button for unapplied remote clipboard items
            if entry.isApplicable {
                Button {
                    applying = true
                    Task {
                        await store.applyClipboard(entry: entry)
                        applying = false
                    }
                } label: {
                    if applying {
                        ProgressView().controlSize(.mini)
                    } else {
                        Label("Apply", systemImage: "doc.on.clipboard")
                            .font(.caption)
                    }
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.mini)
                .disabled(applying)
            } else if entry.applied_locally {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
                    .font(.caption)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .contentShape(Rectangle())
        .onTapGesture {
            if entry.text_preview != nil { expanded.toggle() }
        }
    }

    private var kindIcon: String {
        switch entry.kind {
        case "remote_clipboard_available", "clipboard_text":   return "doc.on.clipboard"
        case "clipboard_image":                                return "photo"
        case "file_transfer_started":                          return "arrow.down.circle"
        case "file_transfer_complete":                         return "checkmark.circle.fill"
        case "file_transfer_failed":                           return "xmark.circle"
        case "peer_connected":                                 return "wifi"
        case "peer_disconnected":                              return "wifi.slash"
        case "sync_paused":                                    return "pause.circle"
        case "sync_resumed":                                   return "play.circle"
        case "clipboard_applied":                              return "checkmark.circle"
        default:                                               return "info.circle"
        }
    }

    private var kindColor: Color {
        switch entry.kind {
        case "remote_clipboard_available":  return entry.applied_locally ? .green : .accentColor
        case "file_transfer_complete":      return .green
        case "file_transfer_failed":        return .red
        case "peer_connected":              return .green
        case "peer_disconnected":           return .secondary
        case "sync_paused":                 return .orange
        default:                            return .secondary
        }
    }

    private func formattedTime(_ ms: Int64) -> String {
        let date = Date(timeIntervalSince1970: Double(ms) / 1000.0)
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

// ── Settings pane for clipboard UX preferences ────────────────────────────────

struct ClipboardPolicyView: View {
    @EnvironmentObject var store: ClipRelayStore
    @State private var timelineFirst = true
    @State private var autoApply = false
    @State private var saving = false

    var body: some View {
        Form {
            Section {
                Toggle("Timeline-first mode", isOn: $timelineFirst)
                    .onChange(of: timelineFirst) { newValue in
                        Task { await store.setTimelineFirstMode(enabled: newValue) }
                    }
                Text("Remote clipboard items appear in the feed instead of automatically overwriting your clipboard. You tap Apply to use them.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            } header: {
                Text("Clipboard Behavior")
            }

            if timelineFirst {
                Section {
                    Toggle("Auto-apply from trusted devices", isOn: $autoApply)
                        .onChange(of: autoApply) { newValue in
                            Task { await store.setAutoApplyClipboard(enabled: newValue) }
                        }
                    Text("When enabled, clipboard items from trusted devices are still applied automatically. Timeline-first for all others.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                } header: {
                    Text("Auto-Apply (optional)")
                }
            }
        }
        .formStyle(.grouped)
        .padding()
        .onAppear {
            timelineFirst = store.clipboardPolicy.timelineFirstMode
            autoApply = store.clipboardPolicy.autoApply
        }
    }
}
