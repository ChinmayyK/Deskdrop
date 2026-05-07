import AppKit
import Combine
import Foundation
import SwiftUI
import UserNotifications

@MainActor
final class ClipRelayStore: ObservableObject {
    @Published var status: AppStatusSnapshot?
    @Published var devices: [ManagedDevice] = []
    @Published var timeline: [TimelineItem] = []
    @Published var feedback: [FeedbackEventSnapshot] = []
    @Published var settings: ClipRelaySettingsSnapshot?
    @Published var toasts: [ToastItem] = []
    @Published var selectedSection: DashboardSection = .timeline
    @Published var quickSendContext: QuickSendContext?
    @Published var pendingTrustRequest: DeviceDetailSnapshot?
    @Published var commandInput: String = ""
    @Published var connectionBanner: String = "Starting…"
    @Published var currentClipboardText: String = ""
    @Published var manualConnectAddress: String = ""

    private let ipc = ClipRelayIPCClient.shared
    private let watcher = ClipboardWatcher()
    private lazy var setter = ClipboardSetter(watcher: watcher)
    private var daemonProcess: Process?
    private var refreshTask: Task<Void, Never>?
    private var feedbackTask: Task<Void, Never>?
    private var seenFeedbackIds = Set<String>()

    func start() {
        requestNotificationPermission()
        ensureDaemonIsRunning()
        bindClipboardWatcher()
        watcher.start()
        refreshAll()
        startRefreshLoops()
    }

    func stop() {
        watcher.stop()
        refreshTask?.cancel()
        feedbackTask?.cancel()
    }

    var connectedDevices: [ManagedDevice] {
        devices.filter(\.isConnected)
    }

    var trustedDevices: [ManagedDevice] {
        devices.filter { $0.trustState == .trusted }
    }

    var statusSummary: String {
        guard let status else { return "Daemon unavailable" }
        let sync = status.syncEnabled ? "Sync on" : "Sync off"
        return "\(status.peerCount) device\(status.peerCount == 1 ? "" : "s") connected · \(sync)"
    }

    func refreshAll() {
        Task {
            async let statusValue = try? ipc.status()
            async let historyValue = try? ipc.history(limit: settings?.historyLimit ?? 80)
            async let trustedValue = try? ipc.trustedDevices()
            async let settingsValue = try? ipc.settings()

            let resolvedSettings = await settingsValue
            if let resolvedSettings {
                self.settings = resolvedSettings
            }

            let resolvedStatus = await statusValue
            let resolvedHistory = await historyValue
            let resolvedTrusted = await trustedValue

            if let resolvedStatus {
                self.status = resolvedStatus
                self.connectionBanner = resolvedStatus.syncEnabled ? "Ready" : "Sync paused"
            } else {
                self.connectionBanner = "Waiting for daemon"
            }

            if let resolvedHistory {
                self.timeline = resolvedHistory.sorted { $0.timestamp > $1.timestamp }
            }

            self.devices = self.mergeDevices(
                peers: resolvedStatus?.peers ?? [],
                trusted: resolvedTrusted ?? []
            )
        }
    }

    func openQuickSendOverlay(for text: String) {
        quickSendContext = QuickSendContext(text: text)
        Task {
            try? await Task.sleep(for: .seconds(4))
            if self.quickSendContext?.text == text {
                self.quickSendContext = nil
            }
        }
    }

    func sendCurrentClipboard(to target: ManagedDevice?) {
        guard !currentClipboardText.isEmpty else { return }
        Task { await self.sendText(currentClipboardText, target: target?.name) }
    }

    private func sendText(_ text: String, target: String?) async {
        do {
            _ = try await ipc.pushText(text, target: target)
            let destination = target ?? "all devices"
            showToast(title: "Sent", body: "Sent to \(destination)", tint: .green)
            refreshAll()
        } catch {
            showToast(title: "Send failed", body: error.localizedDescription, tint: .red)
        }
    }

    func copyTimelineItem(_ item: TimelineItem) {
        guard let text = item.fullText else { return }
        setter.setText(text)
        currentClipboardText = text
        showToast(title: "Copied", body: "Placed on this Mac", tint: .blue)
    }

    func sendTimelineItem(_ item: TimelineItem, to target: ManagedDevice?) {
        Task {
            do {
                _ = try await ipc.historyRepush(id: item.id, target: target?.name)
                let destination = target?.name ?? "all devices"
                showToast(title: "Sent", body: "Sent to \(destination)", tint: .green)
                refreshAll()
            } catch {
                showToast(title: "Send failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func pinTimelineItem(_ item: TimelineItem, pinned: Bool) {
        Task {
            do {
                try await ipc.historyPin(id: item.id, pinned: pinned)
                refreshAll()
            } catch {
                showToast(title: "Update failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func deleteTimelineItem(_ item: TimelineItem) {
        Task {
            do {
                try await ipc.historyDelete(id: item.id)
                refreshAll()
            } catch {
                showToast(title: "Delete failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func connectManual() {
        let trimmed = manualConnectAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let parts = trimmed.split(separator: ":")
        let host = String(parts[0])
        let port = parts.count > 1 ? Int(parts[1]) ?? 47823 : 47823
        Task {
            do {
                try await ipc.connect(ip: host, port: port)
                showToast(title: "Connecting", body: "Trying \(trimmed)", tint: .orange)
                manualConnectAddress = ""
                refreshAll()
            } catch {
                showToast(title: "Connect failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func disconnect(_ device: ManagedDevice) {
        Task {
            do {
                try await ipc.disconnect(deviceId: device.id)
                showToast(title: "Disconnected", body: device.name, tint: .orange)
                refreshAll()
            } catch {
                showToast(title: "Disconnect failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func trust(_ device: ManagedDevice) {
        Task {
            do {
                try await ipc.trust(deviceId: device.id)
                pendingTrustRequest = nil
                refreshAll()
            } catch {
                showToast(title: "Trust failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func reject(_ device: ManagedDevice) {
        Task {
            do {
                try await ipc.reject(deviceId: device.id)
                pendingTrustRequest = nil
                refreshAll()
            } catch {
                showToast(title: "Reject failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func revoke(_ device: ManagedDevice) {
        Task {
            do {
                try await ipc.revoke(deviceId: device.id)
                refreshAll()
            } catch {
                showToast(title: "Revoke failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func rename(_ device: ManagedDevice, to newName: String) {
        let trimmed = newName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        Task {
            do {
                try await ipc.rename(deviceId: device.id, displayName: trimmed)
                refreshAll()
            } catch {
                showToast(title: "Rename failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func saveSettings(_ updated: ClipRelaySettingsSnapshot) {
        let patch: [String: Any] = [
            "device_name": updated.deviceName,
            "sync_enabled": updated.syncEnabled,
            "sync_text": updated.syncText,
            "sync_images": updated.syncImages,
            "sync_files": updated.syncFiles,
            "sync_mode": updated.syncMode.rawValue,
            "max_payload_bytes": updated.maxPayloadBytes,
            "history_limit": updated.historyLimit,
            "max_history_text_bytes": updated.maxHistoryTextBytes,
            "show_receive_notification": updated.showReceiveNotification,
            "require_tofu_confirmation": updated.requireTofuConfirmation,
            "block_sensitive_text": updated.blockSensitiveText,
            "ignore_patterns": updated.ignorePatterns,
            "clipboard_poll_ms": updated.clipboardPollMs,
            "smart_sync_duplicate_window_ms": updated.smartSyncDuplicateWindowMs,
            "smart_sync_debounce_ms": updated.smartSyncDebounceMs,
            "port": updated.port,
            "start_on_login": updated.startOnLogin,
        ]

        Task {
            do {
                try await ipc.patchSettings(patch)
                settings = updated
                refreshAll()
                showToast(title: "Saved", body: "Preferences updated", tint: .green)
            } catch {
                showToast(title: "Save failed", body: error.localizedDescription, tint: .red)
            }
        }
    }

    func performCommand(_ command: String) {
        let trimmed = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        if trimmed == "/history" {
            selectedSection = .timeline
            return
        }
        if trimmed == "/devices" {
            selectedSection = .devices
            return
        }
        if trimmed.hasPrefix("/send ") {
            let targetName = String(trimmed.dropFirst("/send ".count))
            if let device = devices.first(where: { $0.name.localizedCaseInsensitiveContains(targetName) }) {
                sendCurrentClipboard(to: device)
            }
            return
        }
        if trimmed.hasPrefix("/connect ") {
            manualConnectAddress = String(trimmed.dropFirst("/connect ".count))
            connectManual()
            return
        }
        if trimmed.hasPrefix("/trust ") {
            let targetName = String(trimmed.dropFirst("/trust ".count))
            if let device = devices.first(where: { $0.name.localizedCaseInsensitiveContains(targetName) }) {
                trust(device)
            }
        }
    }

    private func bindClipboardWatcher() {
        watcher.onTextChange = { [weak self] text in
            Task { @MainActor in
                self?.handleLocalClipboardText(text)
            }
        }
        watcher.onImageChange = { [weak self] data, mime in
            Task { @MainActor in
                self?.handleLocalClipboardImage(data, mimeType: mime)
            }
        }
        watcher.onFileChange = { [weak self] url in
            Task { @MainActor in
                self?.handleLocalClipboardFile(url)
            }
        }
    }

    private func handleLocalClipboardText(_ text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return }
        currentClipboardText = normalized

        guard let settings else { return }

        Task {
            do {
                if settings.syncMode == .manual || !settings.syncEnabled {
                    _ = try await ipc.rememberText(normalized)
                    showToast(title: "Saved", body: "Added to timeline", tint: .blue)
                } else {
                    _ = try await ipc.pushText(normalized, target: nil)
                }
                refreshAll()
            } catch {
                showToast(title: "Sync skipped", body: error.localizedDescription, tint: .orange)
            }
        }
    }

    private func handleLocalClipboardImage(_ data: Data, mimeType: String) {
        guard let settings, settings.syncEnabled else { return }
        guard settings.syncMode != .manual else {
            showToast(title: "Image copied", body: "Auto sync is paused in manual mode", tint: .orange)
            return
        }

        Task {
            do {
                _ = try await ipc.pushImage(mime: mimeType, data: data)
                refreshAll()
            } catch {
                showToast(title: "Image sync skipped", body: error.localizedDescription, tint: .orange)
            }
        }
    }

    private func handleLocalClipboardFile(_ url: URL) {
        guard let settings, settings.syncEnabled, settings.syncFiles else { return }
        guard settings.syncMode != .manual else {
            showToast(title: "File copied", body: "Auto sync is paused in manual mode", tint: .orange)
            return
        }

        Task {
            do {
                let data = try Data(contentsOf: url, options: .mappedIfSafe)
                _ = try await ipc.pushFile(name: url.lastPathComponent, data: data)
                refreshAll()
            } catch {
                showToast(title: "File sync skipped", body: error.localizedDescription, tint: .orange)
            }
        }
    }

    private func startRefreshLoops() {
        feedbackTask?.cancel()
        refreshTask?.cancel()

        feedbackTask = Task {
            while !Task.isCancelled {
                await pollFeedback()
                try? await Task.sleep(for: .milliseconds(350))
            }
        }

        refreshTask = Task {
            while !Task.isCancelled {
                await refreshStatusAndDevices()
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    private func refreshStatusAndDevices() async {
        do {
            async let statusValue = ipc.status()
            async let trustedValue = ipc.trustedDevices()
            async let settingsValue = ipc.settings()

            let resolvedStatus = try await statusValue
            let resolvedTrusted = try await trustedValue
            let resolvedSettings = try await settingsValue

            status = resolvedStatus
            settings = resolvedSettings
            devices = mergeDevices(peers: resolvedStatus.peers, trusted: resolvedTrusted)
            connectionBanner = resolvedStatus.peerCount == 0 ? "No devices connected" : "Connected"
        } catch {
            connectionBanner = error.localizedDescription
        }
    }

    private func refreshHistory() async {
        do {
            let items = try await ipc.history(limit: settings?.historyLimit ?? 80)
            timeline = items.sorted { $0.timestamp > $1.timestamp }
        } catch {
            showToast(title: "History unavailable", body: error.localizedDescription, tint: .orange)
        }
    }

    private func pollFeedback() async {
        do {
            let events = try await ipc.events(limit: 50)
            let newEvents = events.filter { seenFeedbackIds.insert($0.id).inserted }
                .sorted { $0.timestamp < $1.timestamp }
            guard !newEvents.isEmpty else { return }

            feedback = (events + feedback).uniqued(by: \.id).sorted { $0.timestamp > $1.timestamp }

            var shouldRefreshHistory = false
            var shouldRefreshDevices = false

            for event in newEvents {
                switch event.kind {
                case "clipboard_received":
                    shouldRefreshHistory = true
                    if let clipboardId = event.clipboardId {
                        await applyIncomingClipboard(clipboardId)
                    }
                    showToast(title: "Clipboard received", body: event.message, tint: .blue)
                    if settings?.showReceiveNotification == true {
                        sendSystemNotification(title: "ClipRelay", body: event.message)
                    }
                case "clipboard_synced", "clipboard_dispatch":
                    showToast(title: "Sent", body: event.message, tint: .green)
                case "clipboard_sync_failed", "warning":
                    showToast(title: "Issue", body: event.message, tint: .red)
                case "peer_connected", "peer_disconnected":
                    shouldRefreshDevices = true
                    showToast(title: "Network", body: event.message, tint: .orange)
                case "history_metadata":
                    shouldRefreshHistory = true
                case "trust_prompt":
                    shouldRefreshDevices = true
                    if let deviceId = event.deviceId {
                        pendingTrustRequest = try? await ipc.deviceDetails(id: deviceId)
                    }
                default:
                    break
                }
            }

            if shouldRefreshDevices {
                await refreshStatusAndDevices()
            }
            if shouldRefreshHistory {
                await refreshHistory()
            }
        } catch {
            connectionBanner = error.localizedDescription
        }
    }

    private func applyIncomingClipboard(_ clipboardId: UInt64) async {
        do {
            let incoming = try await ipc.incomingClipboard(id: clipboardId)
            switch incoming.payload {
            case .text(let text):
                setter.setText(text)
                currentClipboardText = text
            case .image(let mime, let dataBase64):
                guard let data = Data(base64Encoded: dataBase64) else { return }
                setter.setImage(data, mimeType: mime)
            case .file(let name, let dataBase64):
                guard let data = Data(base64Encoded: dataBase64) else { return }
                let url = try materializeIncomingFile(named: name, data: data)
                setter.setFileURL(url)
            }
        } catch {
            showToast(title: "Apply failed", body: error.localizedDescription, tint: .red)
        }
    }

    private func materializeIncomingFile(named name: String, data: Data) throws -> URL {
        let fm = FileManager.default
        let downloads = fm.urls(for: .downloadsDirectory, in: .userDomainMask).first
        let root = (downloads ?? fm.temporaryDirectory).appendingPathComponent("ClipRelay", isDirectory: true)
        try fm.createDirectory(at: root, withIntermediateDirectories: true, attributes: nil)

        let safeName = sanitizeFileName(name)
        var destination = root.appendingPathComponent(safeName)
        let stem = destination.deletingPathExtension().lastPathComponent
        let ext = destination.pathExtension
        var counter = 2
        while fm.fileExists(atPath: destination.path) {
            let candidate = ext.isEmpty ? "\(stem) \(counter)" : "\(stem) \(counter).\(ext)"
            destination = root.appendingPathComponent(candidate)
            counter += 1
        }

        try data.write(to: destination, options: .atomic)
        return destination
    }

    private func sanitizeFileName(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        let replaced = trimmed.replacingOccurrences(of: "/", with: "-")
            .replacingOccurrences(of: ":", with: "-")
        return replaced.isEmpty ? "ClipRelay File" : replaced
    }

    private func mergeDevices(
        peers: [PeerSnapshot],
        trusted: [TrustedDeviceSnapshot]
    ) -> [ManagedDevice] {
        var merged: [String: ManagedDevice] = [:]

        for device in trusted {
            merged[device.deviceId] = ManagedDevice(
                id: device.deviceId,
                name: device.effectiveName,
                rawName: device.deviceName,
                endpoint: nil,
                connectionState: .disconnected,
                trustState: device.state,
                fingerprint: device.shortFingerprint,
                lastSeen: device.lastSeen,
                lastSync: nil,
                lastError: nil
            )
        }

        for peer in peers {
            var value = merged[peer.id] ?? ManagedDevice(
                id: peer.id,
                name: peer.friendlyName,
                rawName: peer.friendlyName,
                endpoint: nil,
                connectionState: peer.status,
                trustState: peer.trusted ? .trusted : .untrusted,
                fingerprint: nil,
                lastSeen: peer.lastSeen,
                lastSync: peer.lastSync,
                lastError: peer.lastError
            )
            value.name = value.name.isEmpty ? peer.friendlyName : value.name
            value.rawName = peer.friendlyName
            value.endpoint = peer.ip.map { "\($0):\(peer.port)" }
            value.connectionState = peer.status
            value.trustState = peer.trusted ? .trusted : value.trustState
            value.lastSeen = peer.lastSeen ?? value.lastSeen
            value.lastSync = peer.lastSync
            value.lastError = peer.lastError
            merged[peer.id] = value
        }

        return merged.values.sorted { lhs, rhs in
            if lhs.connectionState == .connected && rhs.connectionState != .connected { return true }
            if lhs.connectionState != .connected && rhs.connectionState == .connected { return false }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }
    }

    private func ensureDaemonIsRunning() {
        Task {
            do {
                try await ipc.ping()
            } catch {
                launchDaemonProcess()
            }
        }
    }

    private func launchDaemonProcess() {
        guard daemonProcess == nil else { return }
        guard let executable = daemonExecutableURL() else {
            connectionBanner = "Couldn’t find cliprelay-daemon"
            return
        }

        let process = Process()
        process.executableURL = executable
        process.arguments = []
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try? process.run()
        daemonProcess = process
        connectionBanner = "Launching daemon…"
    }

    private func daemonExecutableURL() -> URL? {
        let fm = FileManager.default
        let repoRoot = Bundle.main.bundleURL
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let candidates: [String] = [
            Bundle.main.bundlePath + "/Contents/MacOS/cliprelay-daemon",
            Bundle.main.bundlePath + "/Contents/Resources/cliprelay-daemon",
            "/usr/local/bin/cliprelay-daemon",
            "/opt/homebrew/bin/cliprelay-daemon",
            repoRoot.appendingPathComponent("target/debug/cliprelay-daemon").path,
            repoRoot.appendingPathComponent("target/release/cliprelay-daemon").path,
        ]
        return candidates
            .map(URL.init(fileURLWithPath:))
            .first(where: { fm.isExecutableFile(atPath: $0.path) })
    }

    private func showToast(title: String, body: String, tint: Color) {
        let toast = ToastItem(title: title, body: body, tint: tint)
        toasts.append(toast)
        Task {
            try? await Task.sleep(for: .seconds(3))
            await MainActor.run {
                self.toasts.removeAll { $0.id == toast.id }
            }
        }
    }

    private func requestNotificationPermission() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    private func sendSystemNotification(title: String, body: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}

private extension Array {
    func uniqued<T: Hashable>(by keyPath: KeyPath<Element, T>) -> [Element] {
        var seen = Set<T>()
        return filter { seen.insert($0[keyPath: keyPath]).inserted }
    }
}
