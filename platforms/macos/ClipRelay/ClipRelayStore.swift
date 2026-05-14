// ClipRelay — macOS app state store
// Bridges IPC responses to SwiftUI view models.
// Enforces: no raw UUIDs in public-facing state.

import Foundation
import Combine
import AppKit

@MainActor
final class ClipRelayStore: ObservableObject {

    // ── Core state ────────────────────────────────────────────────────────────
    @Published var peers: [PeerViewModel] = []
    @Published var activeTrustPrompt: TrustPrompt? = nil
    @Published var isRunning: Bool = false
    @Published var statusLine: String = "Starting…"
    @Published var connectedCount: Int = 0

    // ── Activity feed ─────────────────────────────────────────────────────────
    @Published var activityFeed: [IpcActivityEntry] = []
    @Published var activeTransfers: [FileTransferState] = []
    @Published var clipboardPolicy = ClipboardPolicy()

    // ── Dashboard UI state ────────────────────────────────────────────────────
    @Published var selectedSection: DashboardSection = .timeline
    @Published var toasts: [ToastItem] = []
    @Published var manualConnectAddress: String = ""
    @Published var settings: ClipRelaySettingsSnapshot? = nil
    @Published var quickSendContext: QuickSendContext? = nil
    @Published var pendingTrustRequest: DeviceDetailSnapshot? = nil
    @Published var dashboardStatus: StatusSnapshot? = nil
    @Published var pinnedItemIds: Set<Int64> = []

    private var lastActivityId: Int64 = 0
    private let ipc: ClipRelayIPCClient
    private var pollTimer: Timer?
    private var pendingRename: PeerViewModel? = nil
    private var toastWorkItems: [UUID: DispatchWorkItem] = [:]

    init(ipc: ClipRelayIPCClient = .shared) {
        self.ipc = ipc
        startPolling()
    }

    // MARK: - Computed / Bridging

    var connectionBanner: String { statusLine }
    var devices: [ManagedDevice] { peers.map(ManagedDevice.init) }
    var connectedDevices: [ManagedDevice] { devices.filter(\.isConnected) }
    var status: StatusSnapshot? { dashboardStatus }

    var timeline: [TimelineItem] {
        activityFeed.prefix(80).map { TimelineItem(entry: $0, pinned: pinnedItemIds.contains($0.id)) }
    }

    // MARK: - Lifecycle

    func start() { startPolling() }
    func stop()  { pollTimer?.invalidate(); pollTimer = nil }

    private func startPolling() {
        pollTimer = Timer.scheduledTimer(withTimeInterval: 1.5, repeats: true) { [weak self] _ in
            Task { await self?.refresh() }
        }
    }

    func refresh() async {
        do {
            let s = try await ipc.status()
            isRunning      = true
            connectedCount = s.peers.filter { $0.status == "connected" }.count
            statusLine     = connectedCount == 0
                ? "Ready — no devices nearby"
                : "\(connectedCount) device\(connectedCount == 1 ? "" : "s") connected"
            peers = s.peers.map { makePeerViewModel($0) }
            dashboardStatus = StatusSnapshot(
                peerCount:    connectedCount,
                trustedCount: s.peers.filter { $0.trusted }.count,
                lastSyncAt:   s.peers.compactMap { $0.last_sync }
                    .max().map { Date(timeIntervalSince1970: TimeInterval($0)) },
                syncEnabled:  true,
                daemonVersion: nil
            )
        } catch {
            isRunning       = false
            statusLine      = "Daemon not running"
            dashboardStatus = nil
        }
    }

    // MARK: - Device actions (ManagedDevice variants)

    func disconnect(_ device: ManagedDevice) {
        Task { try? await ipc.disconnectPeer(deviceId: device.id); await refresh() }
    }
    func trust(_ device: ManagedDevice) {
        Task { try? await ipc.approveTrust(deviceId: device.id, deviceName: device.name, pubkeyBytes: Data()); await refresh() }
    }
    func reject(_ device: ManagedDevice) {
        Task { try? await ipc.rejectTrust(deviceId: device.id); await refresh() }
    }
    func revoke(_ device: ManagedDevice) {
        Task { try? await ipc.revokeDevice(deviceId: device.id); await refresh() }
    }
    func rename(_ device: ManagedDevice, to newName: String) {
        Task { try? await ipc.renameDevice(deviceId: device.id, displayName: newName); await refresh() }
    }

    // MARK: - Device actions (PeerViewModel variants)

    func pauseSync(_ peer: PeerViewModel)        { Task { try? await ipc.pauseSync(deviceId: peer.id);     await refresh() } }
    func resumeSync(_ peer: PeerViewModel)       { Task { try? await ipc.resumeSync(deviceId: peer.id);    await refresh() } }
    func forgetDevice(_ peer: PeerViewModel)     { Task { try? await ipc.forgetDevice(deviceId: peer.id);  await refresh() } }
    func revokeTrust(_ peer: PeerViewModel)      { Task { try? await ipc.revokeDevice(deviceId: peer.id);  await refresh() } }
    func disconnect(_ peer: PeerViewModel)       { Task { try? await ipc.disconnectPeer(deviceId: peer.id); await refresh() } }
    func toggleAutoConnect(_ peer: PeerViewModel) {
        Task { try? await ipc.setAutoConnect(deviceId: peer.id, enabled: !peer.autoConnect); await refresh() }
    }

    func beginRename(_ peer: PeerViewModel) {
        pendingRename = peer
        NotificationCenter.default.post(name: .beginRename, object: peer)
    }
    func applyRename(deviceId: String, newName: String) {
        Task { try? await ipc.renameDevice(deviceId: deviceId, displayName: newName); await refresh() }
    }

    func connectManual() {
        let addr = manualConnectAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return }
        Task {
            try? await ipc.connectManual(address: addr)
            await refresh()
            showToast(title: "Connecting…", body: addr, tint: PBTheme.accentBlue)
        }
    }

    func sendFile(url: URL, to device: ManagedDevice?) {
        Task { _ = try? await ipc.sendFile(url: url, targetDeviceId: device?.id) }
    }
    func sendFile(url: URL, toPeer deviceId: String? = nil) {
        Task { _ = try? await ipc.sendFile(url: url, targetDeviceId: deviceId) }
    }

    // MARK: - Trust prompts (legacy TrustPrompt model)

    func approveTrust(_ prompt: TrustPrompt) {
        Task {
            try? await ipc.approveTrust(deviceId: prompt.deviceId, deviceName: prompt.deviceName, pubkeyBytes: prompt.publicKeyBytes)
            activeTrustPrompt = nil; await refresh()
        }
    }
    func rejectTrust(_ prompt: TrustPrompt) {
        Task { try? await ipc.rejectTrust(deviceId: prompt.deviceId); activeTrustPrompt = nil }
    }

    // MARK: - Timeline

    func copyTimelineItem(_ item: TimelineItem) {
        if let text = item.fullText {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            showToast(title: "Copied", body: String(text.prefix(50)), tint: PBTheme.accentGreen)
        }
        Task {
            if let entry = activityFeed.first(where: { $0.id == item.id }) {
                await applyClipboard(entry: entry)
            }
        }
    }

    func sendTimelineItem(_ item: TimelineItem, to device: ManagedDevice?) {
        Task {
            guard let entry = activityFeed.first(where: { $0.id == item.id }),
                  let hash = entry.content_hash else { return }
            try? await ipc.sendClipboardByHash(hash: hash, targetDeviceId: device?.id)
            let target = device?.name ?? "all devices"
            showToast(title: "Sent", body: "Clipboard sent to \(target)", tint: PBTheme.accentBlue)
        }
    }

    func pinTimelineItem(_ item: TimelineItem, pinned: Bool) {
        if pinned { pinnedItemIds.insert(item.id) } else { pinnedItemIds.remove(item.id) }
    }

    func deleteTimelineItem(_ item: TimelineItem) {
        activityFeed.removeAll { $0.id == item.id }
        pinnedItemIds.remove(item.id)
    }

    func addTimelineEntry(_ entry: TimelineEntry) {
        // Legacy: synthesise a minimal IpcActivityEntry and prepend
        let synthetic = IpcActivityEntry(
            id: Int64(Date().timeIntervalSince1970 * 1000),
            timestamp_ms: Int64(entry.timestamp.timeIntervalSince1970 * 1000),
            device_id: "",
            device_name: entry.deviceName,
            kind: entry.kind.rawValue,
            summary: entry.preview,
            content_hash: nil,
            text_preview: entry.preview,
            file_name: nil,
            file_bytes: nil,
            transfer_id: nil,
            applied_locally: false,
            relay_path: []
        )
        activityFeed.insert(synthetic, at: 0)
        if activityFeed.count > 200 { activityFeed.removeLast() }
    }

    func sendCurrentClipboard(to device: ManagedDevice?) {
        Task {
            try? await ipc.sendClipboardCurrent(targetDeviceId: device?.id)
            let target = device?.name ?? "all devices"
            showToast(title: "Sent", body: "Clipboard sent to \(target)", tint: PBTheme.accentBlue)
        }
    }

    // MARK: - Activity Feed

    @MainActor
    func refreshActivityFeed() async {
        do {
            let entries    = try await ipc.activityRecent(limit: 100)
            activityFeed   = entries
            lastActivityId = entries.first?.id ?? 0
        } catch {}
    }

    @MainActor
    func pollActivityFeedIncremental() async {
        do {
            let newEntries = try await ipc.activitySince(sinceId: lastActivityId)
            if !newEntries.isEmpty {
                activityFeed.insert(contentsOf: newEntries.reversed(), at: 0)
                if activityFeed.count > 200 { activityFeed = Array(activityFeed.prefix(200)) }
                lastActivityId = newEntries.map(\.id).max() ?? lastActivityId
            }
        } catch {}
    }

    // MARK: - Clipboard policy

    @MainActor
    func applyClipboard(entry: IpcActivityEntry) async {
        guard let hash = entry.content_hash else { return }
        do {
            try await ipc.applyClipboard(contentHash: hash)
            if let idx = activityFeed.firstIndex(where: { $0.id == entry.id }) {
                activityFeed[idx].applied_locally = true
            }
        } catch {}
    }

    @MainActor
    func setTimelineFirstMode(enabled: Bool) async {
        clipboardPolicy.timelineFirstMode = enabled
        try? await ipc.setTimelineFirstMode(enabled: enabled)
    }

    @MainActor
    func setAutoApplyClipboard(enabled: Bool) async {
        clipboardPolicy.autoApply = enabled
        try? await ipc.setAutoApplyClipboard(enabled: enabled)
    }

    // MARK: - Settings

    func saveSettings(_ snapshot: ClipRelaySettingsSnapshot) {
        settings = snapshot
        Task { try? await ipc.saveSettings(snapshot); await refresh() }
        showToast(title: "Settings saved", body: "Changes applied to daemon", tint: PBTheme.accentGreen)
    }

    // MARK: - Command palette

    func performCommand(_ command: String) {
        let cmd = command.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if cmd.hasPrefix("/history") || cmd.hasPrefix("/timeline") { selectedSection = .timeline }
        else if cmd.hasPrefix("/devices") { selectedSection = .devices }
        else if cmd.hasPrefix("/trust")   { selectedSection = .trust }
        else if cmd.hasPrefix("/settings") || cmd.hasPrefix("/prefs") { selectedSection = .settings }
        else if cmd.hasPrefix("/connect ") {
            manualConnectAddress = String(command.dropFirst(9))
            selectedSection = .devices
            connectManual()
        }
    }

    // MARK: - File transfers

    @MainActor func acceptFileTransfer(_ t: FileTransferState) {
        Task { try? await ipc.acceptFileTransfer(transferId: t.id); updateTransferStatus(id: t.id, status: .transferring) }
    }
    @MainActor func rejectFileTransfer(_ t: FileTransferState) {
        Task { try? await ipc.rejectFileTransfer(transferId: t.id); activeTransfers.removeAll { $0.id == t.id } }
    }
    @MainActor func cancelFileTransfer(_ t: FileTransferState) {
        Task { try? await ipc.cancelFileTransfer(transferId: t.id); activeTransfers.removeAll { $0.id == t.id } }
    }

    @MainActor
    func upsertTransfer(_ t: FileTransferState) {
        if let idx = activeTransfers.firstIndex(where: { $0.id == t.id }) { activeTransfers[idx] = t }
        else { activeTransfers.insert(t, at: 0) }
    }

    @MainActor
    private func updateTransferStatus(id: String, status: FileTransferStatus) {
        guard let idx = activeTransfers.firstIndex(where: { $0.id == id }) else { return }
        var t = activeTransfers[idx]
        t = FileTransferState(id: t.id, fromDeviceName: t.fromDeviceName, fileName: t.fileName,
                              totalBytes: t.totalBytes, bytesReceived: t.bytesReceived,
                              percent: t.percent, status: status)
        activeTransfers[idx] = t
    }

    // MARK: - Toast system

    func showToast(title: String, body: String, tint: Color) {
        let toast = ToastItem(title: title, body: body, tint: tint)
        withAnimation(.spring(response: 0.3, dampingFraction: 0.75)) { toasts.append(toast) }
        let work = DispatchWorkItem { [weak self] in
            withAnimation(.easeOut(duration: 0.25)) { self?.toasts.removeAll { $0.id == toast.id } }
            self?.toastWorkItems.removeValue(forKey: toast.id)
        }
        toastWorkItems[toast.id] = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.2, execute: work)
    }

    // MARK: - Mapping

    private func makePeerViewModel(_ raw: IpcPeerRecord) -> PeerViewModel {
        PeerViewModel(
            id:          raw.id,
            displayName: raw.display_name?.isEmpty == false ? raw.display_name! : raw.friendly_name,
            platform:    raw.platform,
            trusted:     raw.trusted,
            remembered:  raw.remembered ?? true,
            connected:   raw.status == "connected",
            syncEnabled: raw.sync_enabled ?? true,
            autoConnect: raw.auto_connect ?? true,
            lastSeen:    raw.last_seen.map { Date(timeIntervalSince1970: TimeInterval($0)) },
            lastSync:    raw.last_sync.map { Date(timeIntervalSince1970: TimeInterval($0)) }
        )
    }
}

extension Notification.Name {
    static let beginRename = Notification.Name("ClipRelayBeginRename")
}
