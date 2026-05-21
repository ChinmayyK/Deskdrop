// Deskdrop — macOS app state store
// Bridges IPC responses to SwiftUI view models.
// Enforces: no raw UUIDs in public-facing state.

import Foundation
import Combine
import AppKit
import SwiftUI
import ServiceManagement

// MARK: - Notification names

extension Notification.Name {
    /// Posted by DeskdropStore.openHistoryPanel() — observed by AppDelegate.
    static let clipRelayOpenHistoryPanel = Notification.Name("com.cliprelay.openHistoryPanel")
    static let clipRelayOpenCommandPalette = Notification.Name("com.cliprelay.openCommandPalette")
    static let clipRelayEnsureDaemon = Notification.Name("com.cliprelay.ensureDaemon")
}

@MainActor
final class DeskdropStore: ObservableObject {

    // ── Core state ────────────────────────────────────────────────────────────
    @Published var peers: [PeerViewModel] = []
    @Published var activeTrustPrompt: TrustPrompt? = nil
    @Published var isRunning: Bool = false
    @Published var statusLine: String = "Starting…"
    @Published var connectedCount: Int = 0
    /// Number of remote clipboard items pending apply — badges the menu bar icon.
    @Published var pendingClipboardCount: Int = 0
    /// This Mac's public-key fingerprint — shown in Security pane for peer verification.
    /// Populated from the daemon status response; nil until first successful poll.
    @Published var localFingerprint: String? = nil

    // ── Activity feed ─────────────────────────────────────────────────────────
    @Published var activityFeed: [IpcActivityEntry] = []
    @Published var activeTransfers: [FileTransferState] = []
    @Published var clipboardPolicy = ClipboardPolicy()

    // ── Dashboard UI state ────────────────────────────────────────────────────
    @Published var selectedSection: DashboardSection = .dashboard
    @Published var toasts: [ToastItem] = []
    @Published var manualConnectAddress: String = ""
    @Published var settings: DeskdropSettingsSnapshot? = nil
    @Published var quickSendContext: QuickSendContext? = nil
    @Published var pendingTrustRequest: DeviceDetailSnapshot? = nil
    @Published var dashboardStatus: StatusSnapshot? = nil
    @Published var pinnedItemIds: Set<Int64> = []
    /// Active phone call from a connected Android device (nil = no active call).
    @Published var activeCall: IncomingCallState? = nil
    private var suppressCallUpdatesUntil: Date? = nil
    /// Battery levels for connected peer devices.
    @Published var peerBatteries: [DeviceBatteryState] = []

    private var lastActivityId: Int64 = 0
    private var lastMirroredAutoAppliedEntryId: Int64 = 0
    private let ipc: DeskdropIPCClient
    private var pollTimer: Timer?
    private var pendingRename: PeerViewModel? = nil
    private var toastWorkItems: [UUID: DispatchWorkItem] = [:]
    private var ipcFailureCount: Int = 0

    init(ipc: DeskdropIPCClient = .shared) {
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

    // ── Clipboard watcher ─────────────────────────────────────────────────────
    // Polls NSPasteboard on a background thread and fires callbacks on change.
    // ClipboardSetter prevents echo: applying a received clipboard increments
    // suppressCount so the watcher skips that event and doesn't re-push it.
    private let watcher = ClipboardWatcher()
    private lazy var setter = ClipboardSetter(watcher: watcher)

    func start() {
        startPolling()
        startWatchingClipboard()
    }

    func stop() {
        pollTimer?.invalidate()
        pollTimer = nil
        watcher.stop()
    }

    // ── Clipboard watcher setup ───────────────────────────────────────────────

    private func startWatchingClipboard() {
        watcher.onTextChange = { [weak self] text in
            self?.handleLocalClipboardText(text)
        }
        watcher.onImageChange = { [weak self] data, mimeType in
            self?.handleLocalClipboardImage(data, mimeType: mimeType)
        }
        watcher.onFileChange = { [weak self] urls in
            self?.handleLocalClipboardFiles(urls)
        }
        watcher.start()
    }

    /// User copied text — update quick-send strip and push to peers if connected.
    private func handleLocalClipboardText(_ text: String) {
        // Always update the quick-send strip (shown in history panel).
        quickSendContext = QuickSendContext(text: text, timestamp: Date())
        guard connectedCount > 0 else { return }
        Task { [weak self] in
            guard let self else { return }
            // push_clipboard tells the daemon to read the OS clipboard itself —
            // avoids double-reading and handles large text more safely than inlining.
            _ = try? await self.ipc.send(cmd: ["cmd": "push_clipboard"])
        }
    }

    private func handleLocalClipboardImage(_ data: Data, mimeType: String) {
        guard connectedCount > 0 else { return }
        Task { [weak self] in
            guard let self else { return }
            _ = try? await self.ipc.send(cmd: [
                "cmd":      "push_clipboard_image",
                "mime":     mimeType,
                "data_b64": data.base64EncodedString(),
            ])
        }
    }

    private func handleLocalClipboardFiles(_ urls: [URL]) {
        guard connectedCount > 0 else { return }
        let readable = urls.filter { FileManager.default.fileExists(atPath: $0.path) }
        guard !readable.isEmpty else { return }
        if readable.count == 1, let first = readable.first {
            sendFile(url: first)
            return
        }

        guard let archiveURL = buildClipboardArchive(from: readable) else {
            showToast(
                title: "Archive failed",
                body: "Could not bundle copied files for transfer",
                tint: CRTheme.accentOrange
            )
            return
        }
        sendFile(url: archiveURL)
        showToast(
            title: "Bundled \(readable.count) files",
            body: "Sending a zip archive to connected devices",
            tint: CRTheme.accentBlue
        )
    }

    /// Apply received clipboard text locally without triggering the watcher callback.
    func applyClipboardLocally(text: String) { setter.setText(text) }
    func applyClipboardImageLocally(_ data: Data, mimeType: String) { setter.setImage(data, mimeType: mimeType) }

    /// Adaptive poll rate:
    ///  • 0.25 s when peers are connected — near-instant sync feedback in the UI
    ///  • 3.0 s when idle — no peers, daemon is quiet, conserve resources
    /// The timer reschedules itself after each tick so the interval adjusts
    /// immediately when the connection state changes.
    private func startPolling() {
        schedulePollTick()
    }

    private func schedulePollTick() {
        pollTimer?.invalidate()
        let interval: TimeInterval = connectedCount > 0 ? 0.25 : 1.0
        pollTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: false) { [weak self] _ in
            Task { @MainActor [weak self] in
                await self?.refresh()
                self?.schedulePollTick()   // reschedule with updated interval
            }
        }
    }

    func refresh() async {
        do {
            let s = try await ipc.status()
            ipcFailureCount = 0
            isRunning      = true
            connectedCount = s.peers.filter { $0.status == "connected" }.count
            let reconnectingCount = s.peers.filter { $0.status == "connecting" }.count
            let reconnectableCount = s.peers.filter {
                $0.status != "connected" &&
                $0.status != "connecting" &&
                $0.trusted &&
                ($0.remembered ?? true) &&
                ($0.auto_connect ?? true)
            }.count
            statusLine = whenStatusLine(
                connectedCount: connectedCount,
                reconnectingCount: reconnectingCount,
                reconnectableCount: reconnectableCount
            )
            var seenIds = Set<String>()
            var uniquePeers = [PeerViewModel]()
            // Deduplicate peers by ID (handles cases where a device is discovered via both IPv4 and IPv6)
            for p in s.peers {
                if !seenIds.contains(p.id) {
                    seenIds.insert(p.id)
                    uniquePeers.append(makePeerViewModel(p))
                }
            }
            peers = uniquePeers
            pendingClipboardCount = s.pending_clipboard_count ?? 0
            if let fp = s.local_fingerprint { localFingerprint = fp }
            
            if let ats = s.active_transfers {
                activeTransfers = ats.map { t in
                    let status: FileTransferStatus = t.status == "paused" ? .paused : .transferring
                    return FileTransferState(
                        id: t.transfer_id,
                        fromDeviceName: t.from_device,
                        fileName: t.file_name,
                        totalBytes: t.bytes_total,
                        bytesReceived: t.bytes_received,
                        percent: t.percent,
                        status: status
                    )
                }
            } else {
                activeTransfers = []
            }

            // ── Call continuity: update active call state ─────────────────────
            // Only mutate when the value actually changes — prevents SwiftUI
            // from tearing down & rebuilding the call banner every 0.25 s,
            // which was silently breaking button hit-testing (mouse-down and
            // mouse-up land on different view instances when the view rebuilds
            // between the two events).
            if let suppress = suppressCallUpdatesUntil, suppress > Date() {
                // Ignore status updates for activeCall during optimistic UI wait
            } else {
                if let call = s.active_call, call.state.lowercased() != "idle" {
                    let incoming = IncomingCallState(
                        deviceId: call.device_id,
                        deviceName: call.device_name,
                        state: call.state,
                        phoneNumber: call.number,
                        contactName: call.contact_name
                    )
                    if activeCall != incoming {
                        activeCall = incoming
                    }
                } else if activeCall != nil {
                    activeCall = nil
                }
            }

            // ── Battery Sync (F20) ────────────────────────────────────────────
            let incomingBatteries = (s.peer_batteries ?? []).map { pb in
                DeviceBatteryState(
                    deviceId: pb.device_id,
                    deviceName: pb.device_name,
                    level: pb.level,
                    charging: pb.charging
                )
            }
            if peerBatteries != incomingBatteries {
                peerBatteries = incomingBatteries
            }

            dashboardStatus = StatusSnapshot(
                peerCount:    connectedCount,
                trustedCount: s.peers.filter { $0.trusted }.count,
                lastSyncAt:   s.peers.compactMap { $0.last_sync }
                    .max().map { Date(timeIntervalSince1970: TimeInterval($0)) },
                syncEnabled:  true,
                daemonVersion: nil
            )
            if lastActivityId > 0 {
                await pollActivityFeedIncremental()
            } else {
                await primeActivityFeed()
            }
        } catch {
            ipcFailureCount += 1
            isRunning       = false
            statusLine      = ipcFailureCount >= 3
                ? "Daemon not running"
                : "Reconnecting to daemon…"
            dashboardStatus = nil
            if case DeskdropIPCError.connectionFailed = error {
                NotificationCenter.default.post(name: .clipRelayEnsureDaemon, object: nil)
            }
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
            do {
                try await ipc.connectManual(address: addr)
                manualConnectAddress = ""
                showToast(title: "Connecting…", body: "Initiated connection to \(addr)", tint: CRTheme.accentBlue)
                // Poll a few times to pick up the peer once the handshake completes.
                try? await Task.sleep(nanoseconds: 800_000_000)
                await refresh()
                try? await Task.sleep(nanoseconds: 1_200_000_000)
                await refresh()
            } catch {
                showToast(
                    title: "Connection failed",
                    body: error.localizedDescription,
                    tint: CRTheme.accentRed
                )
            }
        }
    }

    /// Connect to a specific host IP entered by the user (e.g. from the menu bar dialog).
    func connectManual(host: String) {
        let addr = host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return }
        Task {
            do {
                try await ipc.connectManual(address: addr)
                showToast(title: "Connecting to \(addr)…", body: "Handshake in progress", tint: CRTheme.accentBlue)
                try? await Task.sleep(nanoseconds: 900_000_000)
                await refresh()
                try? await Task.sleep(nanoseconds: 1_500_000_000)
                await refresh()
            } catch {
                showToast(title: "Connection failed", body: error.localizedDescription, tint: CRTheme.accentRed)
            }
        }
    }


    // Single-URL convenience wrappers
    func sendFile(url: URL, to device: ManagedDevice?) {
        sendFiles(urls: [url], to: device)
    }
    func sendFile(url: URL, toPeer deviceId: String? = nil) {
        sendFiles(urls: [url], toPeer: deviceId)
    }

    func sendFiles(urls: [URL], to device: ManagedDevice?) {
        Task {
            for url in urls {
                _ = try? await ipc.sendFile(url: url, targetDeviceId: device?.id)
            }
        }
    }
    func sendFiles(urls: [URL], toPeer deviceId: String? = nil) {
        Task {
            for url in urls {
                _ = try? await ipc.sendFile(url: url, targetDeviceId: deviceId)
            }
        }
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

    // MARK: - Device discovery

    /// Trigger a fresh NSD/mDNS scan. The daemon re-registers its advertisement
    /// and re-starts discovery, picking up any peers that came online recently.
    func scanForDevices() {
        Task {
            _ = try? await ipc.send(cmd: ["cmd": "rescan_peers"])
            // Give discovery 1.5 s to find peers, then refresh the peer list.
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            await refresh()
            showToast(title: "Scan complete", body: "Refreshed peer list", tint: CRTheme.accentBlue)
        }
    }

    /// Reject all currently-untrusted device requests in one action.
    func rejectAll() {
        let untrusted = devices.filter { $0.trustState == .untrusted }
        guard !untrusted.isEmpty else { return }
        Task {
            for device in untrusted {
                try? await ipc.rejectTrust(deviceId: device.id)
            }
            await refresh()
            showToast(title: "Rejected \(untrusted.count)", body: "All pending requests dismissed", tint: CRTheme.accentRed)
        }
    }

    // MARK: - Timeline

    /// Copy item text to pasteboard without marking it applied.
    /// Apply is a separate explicit action (the Apply button / context menu).
    func copyTimelineItem(_ item: TimelineItem) {
        if let text = item.fullText {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            showToast(title: "Copied", body: String(text.prefix(60)), tint: CRTheme.accentGreen)
        }
    }

    func sendTimelineItem(_ item: TimelineItem, to device: ManagedDevice?) {
        Task {
            guard let entry = activityFeed.first(where: { $0.id == item.id }),
                  let hash = entry.content_hash else { return }
            try? await ipc.sendClipboardByHash(hash: hash, targetDeviceId: device?.id)
            let target = device?.name ?? "all devices"
            showToast(title: "Sent", body: "Clipboard sent to \(target)", tint: CRTheme.accentBlue)
        }
    }

    func pinTimelineItem(_ item: TimelineItem, pinned: Bool) {
        if pinned { pinnedItemIds.insert(item.id) } else { pinnedItemIds.remove(item.id) }
    }

    func deleteTimelineItem(_ item: TimelineItem) {
        activityFeed.removeAll { $0.id == item.id }
        pinnedItemIds.remove(item.id)
    }

    /// Synthesise a minimal feed entry from a legacy TimelineEntry.
    /// Uses JSON round-trip via the custom Codable init so we don't need a
    /// memberwise initialiser on IpcActivityEntry.
    func addTimelineEntry(_ entry: TimelineEntry) {
        let dict: [String: Any] = [
            "id":             Int64(Date().timeIntervalSince1970 * 1000),
            "timestamp_ms":   Int64(entry.timestamp.timeIntervalSince1970 * 1000),
            "device_id":      "",
            "device_name":    entry.deviceName,
            "kind":           entry.kind.rawValue,
            "summary":        entry.preview,
            "text_preview":   entry.preview,
            "applied_locally": false,
            "relay_path":     [String]()
        ]
        guard
            let data    = try? JSONSerialization.data(withJSONObject: dict),
            let synthetic = try? JSONDecoder().decode(IpcActivityEntry.self, from: data)
        else { return }
        activityFeed.insert(synthetic, at: 0)
        if activityFeed.count > 200 { activityFeed.removeLast() }
    }

    // MARK: - Command palette actions

    /// Toggle sync on/off — used by command palette ⌘K and menu bar.
    func toggleSync() {
        guard var s = settings else { return }
        s.syncEnabled = !s.syncEnabled
        saveSettings(s)
        let state = s.syncEnabled ? "resumed" : "paused"
        showToast(title: "Sync \(state)", body: s.syncEnabled
            ? "Clipboard sync is now active"
            : "Clipboard sync paused — no events will be forwarded",
            tint: s.syncEnabled ? CRTheme.accentGreen : CRTheme.accentOrange)
    }

    /// Open the Quick Access history panel — triggered by command palette.
    func openHistoryPanel() {
        // Post a notification that AppDelegate listens to — keeps store decoupled from UI.
        NotificationCenter.default.post(name: .clipRelayOpenHistoryPanel, object: nil)
    }

    func openCommandPalette() {
        NotificationCenter.default.post(name: .clipRelayOpenCommandPalette, object: nil)
    }

    /// Send the current local clipboard to all (or one) connected peer.
    func sendCurrentClipboard(to device: ManagedDevice?) {
        Task {
            try? await ipc.sendClipboardCurrent(targetDeviceId: device?.id)
            let target = device?.name ?? "all devices"
            showToast(title: "Sent", body: "Clipboard sent to \(target)", tint: CRTheme.accentBlue)
        }
    }

    func sendQuickContext(to device: ManagedDevice) {
        guard let context = quickSendContext else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(context.text, forType: .string)
        sendCurrentClipboard(to: device)
    }

    private func buildClipboardArchive(from urls: [URL]) -> URL? {
        guard !urls.isEmpty else { return nil }

        let stamp = ISO8601DateFormatter()
            .string(from: Date())
            .replacingOccurrences(of: ":", with: "-")
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("deskdrop-clipboard-archives", isDirectory: true)
        let stagingDir = tempRoot.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let archiveURL = tempRoot.appendingPathComponent("Deskdrop Bundle \(stamp).zip")

        do {
            try FileManager.default.createDirectory(
                at: stagingDir,
                withIntermediateDirectories: true,
                attributes: nil
            )

            var stagedNames = Set<String>()
            for source in urls {
                let stagedName = uniqueClipboardItemName(for: source.lastPathComponent, existing: &stagedNames)
                let stagedURL = stagingDir.appendingPathComponent(stagedName)
                try FileManager.default.createSymbolicLink(at: stagedURL, withDestinationURL: source)
            }

            if FileManager.default.fileExists(atPath: archiveURL.path) {
                try FileManager.default.removeItem(at: archiveURL)
            }

            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
            process.currentDirectoryURL = stagingDir
            process.arguments = ["-r", "-q", archiveURL.path] + stagedNames.sorted()

            try process.run()
            process.waitUntilExit()

            guard process.terminationStatus == 0,
                  FileManager.default.fileExists(atPath: archiveURL.path) else {
                throw NSError(
                    domain: "DeskdropArchive",
                    code: Int(process.terminationStatus),
                    userInfo: nil
                )
            }

            DispatchQueue.global().asyncAfter(deadline: .now() + .seconds(1800)) {
                try? FileManager.default.removeItem(at: archiveURL)
                try? FileManager.default.removeItem(at: stagingDir)
            }
            return archiveURL
        } catch {
            try? FileManager.default.removeItem(at: stagingDir)
            try? FileManager.default.removeItem(at: archiveURL)
            NSLog("Deskdrop: failed to archive clipboard files: \(error.localizedDescription)")
            return nil
        }
    }

    private func uniqueClipboardItemName(for baseName: String, existing: inout Set<String>) -> String {
        guard !existing.contains(baseName) else {
            let stem = URL(fileURLWithPath: baseName).deletingPathExtension().lastPathComponent
            let ext = URL(fileURLWithPath: baseName).pathExtension
            var index = 2
            while true {
                let candidate = ext.isEmpty ? "\(stem) \(index)" : "\(stem) \(index).\(ext)"
                if !existing.contains(candidate) {
                    existing.insert(candidate)
                    return candidate
                }
                index += 1
            }
        }
        existing.insert(baseName)
        return baseName
    }

    // MARK: - Activity Feed

    @MainActor
    func refreshActivityFeed() async {
        do {
            let entries    = try await ipc.activityRecent(limit: 100)
            activityFeed   = entries
            lastActivityId = entries.first?.id ?? 0
            mirrorAutoAppliedClipboardIfNeeded(entries: entries)
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
                mirrorAutoAppliedClipboardIfNeeded(entries: newEntries)
                
                // Only fire user-visible notifications for events the user cares about:
                // remote clipboard items arriving and completed incoming file transfers.
                // Local copies, peer connections, sync events, etc. are silently absorbed
                // into the activity feed without triggering system notifications or toasts.
                for entry in newEntries {
                    switch entry.kind {
                    case "remote_clipboard_available", "file_transfer_complete":
                        NotificationCenter.default.post(name: NSNotification.Name("clipRelayActivityReceived"), object: entry)
                    default:
                        break
                    }
                }
            }
            // Keep pendingClipboardCount in sync with local feed state.
            // This stays accurate between status() polls (which happen less often when idle).
            pendingClipboardCount = activityFeed.filter { $0.isApplicable }.count
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
            // Decrement immediately so the menu bar badge updates without waiting for next poll.
            pendingClipboardCount = max(0, pendingClipboardCount - 1)
            // Apply to local pasteboard via ClipboardSetter (suppresses echo back to peers).
            if let text = entry.text_preview {
                applyClipboardLocally(text: text)
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

    @MainActor
    private func primeActivityFeed() async {
        do {
            let entries = try await ipc.activityRecent(limit: 20)
            activityFeed = entries
            lastActivityId = entries.first?.id ?? 0
            mirrorAutoAppliedClipboardIfNeeded(entries: entries)
            pendingClipboardCount = activityFeed.filter { $0.isApplicable }.count
        } catch {}
    }

    @MainActor
    private func mirrorAutoAppliedClipboardIfNeeded(entries: [IpcActivityEntry]) {
        let pendingMirror = entries
            .filter {
                $0.kind == "remote_clipboard_available" &&
                $0.applied_locally &&
                $0.id > lastMirroredAutoAppliedEntryId &&
                !($0.text_preview?.isEmpty ?? true)
            }
            .sorted { $0.id < $1.id }

        for entry in pendingMirror {
            guard let text = entry.text_preview else { continue }
            applyClipboardLocally(text: text)
            lastMirroredAutoAppliedEntryId = entry.id
        }
    }

    // MARK: - Settings

    func saveSettings(_ snapshot: DeskdropSettingsSnapshot) {
        settings = snapshot
        // startOnLogin is OS-level (LaunchAgent) — handle separately from daemon settings.
        applyLoginItemState(enabled: snapshot.startOnLogin)
        Task {
            do {
                try await ipc.saveSettings(snapshot)
                await refresh()
                showToast(title: "Settings saved", body: "Changes applied", tint: CRTheme.accentGreen)
            } catch {
                showToast(title: "Save failed", body: error.localizedDescription, tint: CRTheme.accentRed)
            }
        }
    }

    /// Registers/unregisters Deskdrop as a login item via SMAppService (macOS 13+).
    private func applyLoginItemState(enabled: Bool) {
        if #available(macOS 13.0, *) {
            let svc = SMAppService.mainApp
            do {
                if enabled  { if svc.status != .enabled  { try svc.register()   } }
                else        { if svc.status == .enabled  { try svc.unregister() } }
            } catch {
                NSLog("Deskdrop: login item \(enabled ? "register" : "unregister") error: \(error)")
            }
        }
    }

    // MARK: - Command palette

    func performCommand(_ command: String) {
        let cmd = command.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if cmd.hasPrefix("/history") || cmd.hasPrefix("/timeline") { selectedSection = .history }
        else if cmd.hasPrefix("/devices") { selectedSection = .devices }
        else if cmd.hasPrefix("/trust")   { selectedSection = .workflows }
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
    func pauseFileTransfer(_ t: FileTransferState) {
        Task { try? await ipc.pauseFileTransfer(transferId: t.id); updateTransferStatus(id: t.id, status: .paused) }
    }
    
    @MainActor
    func resumeFileTransfer(_ t: FileTransferState) {
        Task { try? await ipc.resumeFileTransfer(transferId: t.id); updateTransferStatus(id: t.id, status: .transferring) }
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

    func showToast(
        title: String,
        body: String,
        tint: Color,
        systemImage: String = "sparkles.rectangle.stack",
        detail: String? = nil,
        ttl: TimeInterval? = 4.0,
        progress: Double? = nil,
        primaryAction: ToastAction? = nil,
        secondaryAction: ToastAction? = nil
    ) {
        let toast = ToastItem(
            title: title,
            body: body,
            tint: tint,
            systemImage: systemImage,
            detail: detail,
            ttl: ttl,
            progress: progress,
            primaryAction: primaryAction,
            secondaryAction: secondaryAction
        )
        withAnimation(.spring(response: 0.45, dampingFraction: 0.82, blendDuration: 0.1)) {
            toasts.append(toast)
        }

        guard let ttl else { return }

        let work = DispatchWorkItem { [weak self] in
            self?.dismissToast(id: toast.id)
        }
        toastWorkItems[toast.id] = work
        DispatchQueue.main.asyncAfter(deadline: .now() + ttl, execute: work)
    }

    func dismissToast(id: UUID) {
        toastWorkItems[id]?.cancel()
        toastWorkItems.removeValue(forKey: id)
        withAnimation(.spring(response: 0.45, dampingFraction: 0.82, blendDuration: 0.1)) {
            toasts.removeAll { $0.id == id }
        }
    }

    // MARK: - Call Continuity

    func acceptCall() {
        NSSound.beep()
        guard let call = activeCall else { return }
        suppressCallUpdatesUntil = Date().addingTimeInterval(3.0)
        // Temporarily mark as offhook locally for immediate feedback
        var updated = call
        updated.state = "offhook"
        activeCall = updated
        Task {
            try? await ipc.callAction(action: "accept", targetDevice: call.deviceId)
        }
    }

    func declineCall() {
        NSSound.beep()
        guard let call = activeCall else { return }
        suppressCallUpdatesUntil = Date().addingTimeInterval(3.0)
        withAnimation(.crSpring) { activeCall = nil }
        Task {
            try? await ipc.callAction(action: "decline", targetDevice: call.deviceId)
        }
    }

    func routeAudio(to route: String) {
        guard let call = activeCall else { return }
        Task {
            try? await ipc.callAction(action: "audio_\(route)", targetDevice: call.deviceId)
        }
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
            connectionStatus: raw.status,
            syncEnabled: raw.sync_enabled ?? true,
            autoConnect: raw.auto_connect ?? true,
            lastError:   raw.last_error,
            lastSeen:    raw.last_seen.map { Date(timeIntervalSince1970: TimeInterval($0)) },
            lastSync:    raw.last_sync.map { Date(timeIntervalSince1970: TimeInterval($0)) }
        )
    }

    private func whenStatusLine(connectedCount: Int, reconnectingCount: Int, reconnectableCount: Int) -> String {
        if connectedCount > 0 {
            return "\(connectedCount) device\(connectedCount == 1 ? "" : "s") connected"
        }
        if reconnectingCount > 0 {
            return "Reconnecting to nearby devices…"
        }
        if reconnectableCount > 0 {
            return "Trusted devices ready to reconnect"
        }
        return "Ready — no devices nearby"
    }
}

extension Notification.Name {
    static let beginRename = Notification.Name("DeskdropBeginRename")
}
