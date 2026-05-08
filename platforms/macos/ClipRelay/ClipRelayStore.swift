// ClipRelay — macOS app state store
// Bridges IPC responses to SwiftUI view models.
// Enforces: no raw UUIDs in public-facing state.

import Foundation
import Combine

@MainActor
final class ClipRelayStore: ObservableObject {
    @Published var peers: [PeerViewModel] = []
    @Published var timeline: [TimelineEntry] = []
    @Published var activeTrustPrompt: TrustPrompt? = nil
    @Published var isRunning: Bool = false
    @Published var statusLine: String = "Starting…"
    @Published var connectedCount: Int = 0

    // ── Activity feed (timeline-first clipboard + all events) ─────────────────
    @Published var activityFeed: [IpcActivityEntry] = []
    @Published var activeTransfers: [FileTransferState] = []
    @Published var clipboardPolicy = ClipboardPolicy()

    private var lastActivityId: Int64 = 0

    private let ipc: ClipRelayIPCClient
    private var pollTimer: Timer?
    private var pendingRename: PeerViewModel? = nil

    init(ipc: ClipRelayIPCClient = .shared) {
        self.ipc = ipc
        startPolling()
    }

    // ── Polling ───────────────────────────────────────────────────────────────

    private func startPolling() {
        pollTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { await self?.refresh() }
        }
    }

    func refresh() async {
        do {
            let status = try await ipc.status()
            isRunning = true
            connectedCount = status.peers.filter { $0.status == "connected" }.count
            statusLine = connectedCount == 0
                ? "Ready — no devices nearby"
                : "\(connectedCount) device\(connectedCount == 1 ? "" : "s") connected"
            peers = status.peers.map { makePeerViewModel($0) }
        } catch {
            isRunning = false
            statusLine = "Daemon not running"
        }
    }

    // ── Device lifecycle controls ─────────────────────────────────────────────

    func disconnect(_ peer: PeerViewModel) {
        Task {
            try? await ipc.disconnectPeer(deviceId: peer.id)
            await refresh()
        }
    }

    func pauseSync(_ peer: PeerViewModel) {
        Task {
            try? await ipc.pauseSync(deviceId: peer.id)
            await refresh()
        }
    }

    func resumeSync(_ peer: PeerViewModel) {
        Task {
            try? await ipc.resumeSync(deviceId: peer.id)
            await refresh()
        }
    }

    func forgetDevice(_ peer: PeerViewModel) {
        Task {
            try? await ipc.forgetDevice(deviceId: peer.id)
            await refresh()
        }
    }

    func revokeTrust(_ peer: PeerViewModel) {
        Task {
            try? await ipc.revokeDevice(deviceId: peer.id)
            await refresh()
        }
    }

    func toggleAutoConnect(_ peer: PeerViewModel) {
        Task {
            try? await ipc.setAutoConnect(deviceId: peer.id, enabled: !peer.autoConnect)
            await refresh()
        }
    }

    func beginRename(_ peer: PeerViewModel) {
        pendingRename = peer
        // Trigger a rename sheet — handled by the view
        NotificationCenter.default.post(name: .beginRename, object: peer)
    }

    func applyRename(deviceId: String, newName: String) {
        Task {
            try? await ipc.renameDevice(deviceId: deviceId, displayName: newName)
            await refresh()
        }
    }

    // ── Trust prompts ─────────────────────────────────────────────────────────

    func approveTrust(_ prompt: TrustPrompt) {
        Task {
            try? await ipc.approveTrust(
                deviceId: prompt.deviceId,
                deviceName: prompt.deviceName,
                pubkeyBytes: prompt.publicKeyBytes
            )
            activeTrustPrompt = nil
            await refresh()
        }
    }

    func rejectTrust(_ prompt: TrustPrompt) {
        Task {
            try? await ipc.rejectTrust(deviceId: prompt.deviceId)
            activeTrustPrompt = nil
        }
    }

    // ── Timeline ──────────────────────────────────────────────────────────────

    func addTimelineEntry(_ entry: TimelineEntry) {
        timeline.insert(entry, at: 0)
        if timeline.count > 200 { timeline.removeLast() }
    }

    // ── Activity Feed ─────────────────────────────────────────────────────────

    @MainActor
    func refreshActivityFeed() async {
        do {
            let entries = try await ipc.activityRecent(limit: 100)
            activityFeed = entries
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

    // ── Timeline-first clipboard ──────────────────────────────────────────────

    @MainActor
    func applyClipboard(entry: IpcActivityEntry) async {
        guard let hash = entry.content_hash else { return }
        do {
            try await ipc.applyClipboard(contentHash: hash)
            // Update local feed immediately for snappy UX.
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

    // ── File Transfer ─────────────────────────────────────────────────────────

    func sendFile(url: URL, toPeer deviceId: String? = nil) {
        Task {
            _ = try? await ipc.sendFile(url: url, targetDeviceId: deviceId)
        }
    }

    @MainActor
    func acceptFileTransfer(_ transfer: FileTransferState) {
        Task {
            try? await ipc.acceptFileTransfer(transferId: transfer.id)
            updateTransferStatus(id: transfer.id, status: .transferring)
        }
    }

    @MainActor
    func rejectFileTransfer(_ transfer: FileTransferState) {
        Task {
            try? await ipc.rejectFileTransfer(transferId: transfer.id)
            activeTransfers.removeAll { $0.id == transfer.id }
        }
    }

    @MainActor
    func cancelFileTransfer(_ transfer: FileTransferState) {
        Task {
            try? await ipc.cancelFileTransfer(transferId: transfer.id)
            activeTransfers.removeAll { $0.id == transfer.id }
        }
    }

    @MainActor
    func upsertTransfer(_ t: FileTransferState) {
        if let idx = activeTransfers.firstIndex(where: { $0.id == t.id }) {
            activeTransfers[idx] = t
        } else {
            activeTransfers.insert(t, at: 0)
        }
    }

    @MainActor
    private func updateTransferStatus(id: String, status: FileTransferStatus) {
        if let idx = activeTransfers.firstIndex(where: { $0.id == id }) {
            activeTransfers[idx] = FileTransferState(
                id: activeTransfers[idx].id,
                fromDeviceName: activeTransfers[idx].fromDeviceName,
                fileName: activeTransfers[idx].fileName,
                totalBytes: activeTransfers[idx].totalBytes,
                bytesReceived: activeTransfers[idx].bytesReceived,
                percent: activeTransfers[idx].percent,
                status: status
            )
        }
    }

    // ── View model mapping ────────────────────────────────────────────────────

    private func makePeerViewModel(_ raw: IpcPeerRecord) -> PeerViewModel {
        PeerViewModel(
            id: raw.id,
            // displayName uses the trust store's display_name override if set,
            // otherwise the peer's friendly_name. Never the raw UUID.
            displayName: raw.display_name?.isEmpty == false ? raw.display_name! : raw.friendly_name,
            platform: raw.platform,
            trusted: raw.trusted,
            remembered: raw.remembered ?? true,
            connected: raw.status == "connected",
            syncEnabled: raw.sync_enabled ?? true,
            autoConnect: raw.auto_connect ?? true,
            lastSeen: raw.last_seen.map { Date(timeIntervalSince1970: TimeInterval($0)) },
            lastSync: raw.last_sync.map { Date(timeIntervalSince1970: TimeInterval($0)) }
        )
    }
}

extension Notification.Name {
    static let beginRename = Notification.Name("ClipRelayBeginRename")
}
