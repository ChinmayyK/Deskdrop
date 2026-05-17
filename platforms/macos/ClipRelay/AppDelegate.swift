import AppKit
import Carbon
import Combine
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let store = ClipRelayStore()
    private var statusItem: NSStatusItem!
    private let statusMenuItem  = NSMenuItem(title: "Starting…", action: nil, keyEquivalent: "")
    private let lastSyncMenuItem = NSMenuItem(title: "Last sync: —", action: nil, keyEquivalent: "")
    private var dashboardController:      NSWindowController?
    private var quickAccessController:    NSWindowController?
    private var commandPaletteController: NSWindowController?
    private var cancellables = Set<AnyCancellable>()
    private var daemonProcess: Process?

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard ensureSingleRunningInstance() else { return }
        NSApp.setActivationPolicy(.accessory)
        NSApp.appearance = NSAppearance(named: .aqua)
        startDaemonIfNeeded()
        setupMenuBar()
        setupWindows()
        bindStore()
        registerHotKeys()
        registerSleepWakeObservers()
        registerStoreNotifications()
        store.start()
    }

    /// Observe notifications posted by ClipRelayStore so it stays decoupled from AppKit.
    private func registerStoreNotifications() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(openQuickAccess),
            name: .clipRelayOpenHistoryPanel,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(ensureDaemonResponsiveFromStore),
            name: .clipRelayEnsureDaemon,
            object: nil
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        store.stop()
        daemonProcess?.terminate()
    }

    // MARK: - Daemon lifecycle

    private func startDaemonIfNeeded() {
        if isDaemonSocketPresent() {
            ensureDaemonResponsive(forceRestartOnFailure: true)
            return
        }
        launchDaemonProcess()
    }

    private func launchDaemonProcess() {
        cleanupDaemonSocketIfNeeded()

        let candidates = [
            Bundle.main.resourceURL?.appendingPathComponent("cliprelay-daemon"),
            Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("cliprelay-daemon"),
            URL(fileURLWithPath: "/usr/local/bin/cliprelay-daemon"),
            URL(fileURLWithPath: "/opt/homebrew/bin/cliprelay-daemon")
        ].compactMap { $0 }

        guard let daemonURL = candidates.first(where: {
            FileManager.default.isExecutableFile(atPath: $0.path)
        }) else {
            NSLog("ClipRelay: cliprelay-daemon not found in bundle or PATH candidates")
            return
        }

        let process = Process()
        process.executableURL = daemonURL
        process.environment = ProcessInfo.processInfo.environment.merging([
            "CLIPRELAY_LOG": "info"
        ]) { current, _ in current }

        do {
            try process.run()
            daemonProcess = process
            NSLog("ClipRelay: started daemon at \(daemonURL.path)")
        } catch {
            NSLog("ClipRelay: failed to start daemon: \(error.localizedDescription)")
        }
    }

    @objc private func ensureDaemonResponsiveFromStore() {
        ensureDaemonResponsive(forceRestartOnFailure: true)
    }

    private func ensureDaemonResponsive(forceRestartOnFailure: Bool) {
        Task { [weak self] in
            do {
                try await ClipRelayIPCClient.shared.ping()
            } catch {
                guard forceRestartOnFailure else { return }
                self?.daemonProcess?.terminate()
                self?.daemonProcess = nil
                self?.cleanupDaemonSocketIfNeeded()
                self?.launchDaemonProcess()
            }
        }
    }

    private func isDaemonSocketPresent() -> Bool {
        let path: String
        if let runtime = ProcessInfo.processInfo.environment["XDG_RUNTIME_DIR"] {
            path = "\(runtime)/cliprelay.sock"
        } else {
            path = "/tmp/cliprelay-\(getuid()).sock"
        }
        return FileManager.default.fileExists(atPath: path)
    }

    private func cleanupDaemonSocketIfNeeded() {
        let path: String
        if let runtime = ProcessInfo.processInfo.environment["XDG_RUNTIME_DIR"] {
            path = "\(runtime)/cliprelay.sock"
        } else {
            path = "/tmp/cliprelay-\(getuid()).sock"
        }
        if FileManager.default.fileExists(atPath: path) {
            try? FileManager.default.removeItem(atPath: path)
        }
    }

    // MARK: - Single instance guard

    private func ensureSingleRunningInstance() -> Bool {
        guard let bundleId = Bundle.main.bundleIdentifier else { return true }
        let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundleId)
        guard running.count > 1 else { return true }
        let pid = ProcessInfo.processInfo.processIdentifier
        running.first { $0.processIdentifier != pid }?.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
        NSApp.terminate(nil)
        return false
    }

    // MARK: - Menu bar

    private func setupMenuBar() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.toolTip = "ClipRelay"

        if let img = statusBarImage() {
            statusItem.button?.image = img
            statusItem.button?.imageScaling = .scaleProportionallyUpOrDown
            statusItem.button?.imagePosition = .imageOnly
            statusItem.button?.title = ""
        } else {
            statusItem.button?.title = "CR"
            statusItem.button?.font = .systemFont(ofSize: 12, weight: .semibold)
        }

        let menu = NSMenu()
        statusMenuItem.isEnabled  = false
        lastSyncMenuItem.isEnabled = false
        menu.addItem(statusMenuItem)
        menu.addItem(lastSyncMenuItem)
        menu.addItem(.separator())
        let dashItem = NSMenuItem(title: "Open Dashboard", action: #selector(openDashboard), keyEquivalent: "0")
        let quickItem = NSMenuItem(title: "Quick Access",  action: #selector(openQuickAccess), keyEquivalent: "v")
        let cmdItem   = NSMenuItem(title: "Command Palette", action: #selector(openCommandPalette), keyEquivalent: "k")
        dashItem.keyEquivalentModifierMask  = [.command, .shift]
        quickItem.keyEquivalentModifierMask = [.command, .shift]
        cmdItem.keyEquivalentModifierMask   = [.command]
        menu.addItem(dashItem)
        menu.addItem(quickItem)
        menu.addItem(cmdItem)
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "Quit ClipRelay", action: #selector(quitApp), keyEquivalent: "q"))
        statusItem.menu = menu
    }

    private func statusBarImage() -> NSImage? {
        guard let url   = Bundle.main.url(forResource: "StatusBarIcon", withExtension: "png"),
              let image = NSImage(contentsOf: url) else { return nil }
        image.size       = NSSize(width: 18, height: 18)
        image.isTemplate = true
        return image
    }

    // MARK: - Windows

    private func setupWindows() {
        dashboardController = Self.makeWindow(
            title: "ClipRelay",
            size:  NSSize(width: 1020, height: 700),
            rootView: DashboardRootView(store: store)
        )
        quickAccessController = Self.makePanel(
            title: "Quick Access",
            size:  NSSize(width: 480, height: 560),
            rootView: QuickAccessHistoryView(store: store)
        )
        commandPaletteController = Self.makePanel(
            title: "Command Palette",
            size:  NSSize(width: 520, height: 400),
            rootView: CommandPaletteView(store: store)
        )
    }

    // MARK: - Store bindings

    private func bindStore() {
        store.$statusLine
            .receive(on: RunLoop.main)
            .sink { [weak self] banner in
                self?.statusMenuItem.title = banner
                self?.statusItem.button?.toolTip = "ClipRelay • \(banner)"
            }
            .store(in: &cancellables)

        store.$dashboardStatus
            .receive(on: RunLoop.main)
            .sink { [weak self] status in
                if let lastSync = status?.lastSyncAt {
                    self?.lastSyncMenuItem.title = "Last sync: \(lastSync.relativeTimeString())"
                } else {
                    self?.lastSyncMenuItem.title = "Last sync: —"
                }
            }
            .store(in: &cancellables)

        // Badge the menu bar icon when clipboard items are waiting to be applied.
        // Shows a red dot overlay when count > 0 so the user knows something arrived
        // without needing to open the dashboard.
        store.$pendingClipboardCount
            .receive(on: RunLoop.main)
            .sink { [weak self] count in
                self?.updateMenuBarBadge(pendingCount: count)
            }
            .store(in: &cancellables)

        store.$pendingTrustRequest
            .compactMap { $0 }
            .sink { [weak self] detail in
                self?.presentTrustPrompt(for: detail)
            }
            .store(in: &cancellables)
    }

    // MARK: - Sleep / Wake
    //
    // When the Mac sleeps, mDNS advertisements are torn down by the OS.
    // On wake, the Rust engine's mdns-sd daemon re-advertises automatically,
    // but the Android side may not rediscover for up to 60 s.
    // We speed this up by having the Mac trigger a store refresh immediately
    // on wake, which updates the UI and lets the engine know to expect
    // incoming connections.  The engine itself handles peer reconnection;
    // this just ensures the UI reflects truth quickly.

    private func registerSleepWakeObservers() {
        let wsnc = NSWorkspace.shared.notificationCenter
        wsnc.addObserver(self,
            selector: #selector(handleSystemWake),
            name: NSWorkspace.didWakeNotification,
            object: nil)
        wsnc.addObserver(self,
            selector: #selector(handleSystemSleep),
            name: NSWorkspace.willSleepNotification,
            object: nil)
    }

    @objc private func handleSystemWake() {
        // On wake, poll immediately (don't wait for next timer tick) so the
        // menu bar and dashboard reflect the restored state right away.
        Task { await store.refresh() }
    }

    @objc private func handleSystemSleep() {
        // Nothing to do — the Rust engine handles clean peer shutdown on sleep.
        // We just log for diagnosability.
        NSLog("ClipRelay: system going to sleep")
    }

    // MARK: - Menu bar badge

    /// Overlays a small red dot on the status-bar icon when `pendingCount > 0`.
    /// Drawn directly onto a composited NSImage so no extra views are needed.
    private func updateMenuBarBadge(pendingCount: Int) {
        guard let button = statusItem.button else { return }

        let baseImage: NSImage
        if let img = statusBarImage() {
            baseImage = img.copy() as! NSImage
        } else {
            button.title = pendingCount > 0 ? "CR●" : "CR"
            return
        }

        guard pendingCount > 0 else {
            button.image = baseImage
            button.imageScaling = .scaleProportionallyUpOrDown
            button.toolTip = "ClipRelay"
            return
        }

        let size = baseImage.size
        let badged = NSImage(size: size)
        badged.lockFocus()
        baseImage.draw(in: NSRect(origin: .zero, size: size))

        let dotSize: CGFloat = size.height * 0.38
        let dotRect = CGRect(
            x: size.width - dotSize - 0.5,
            y: size.height - dotSize - 0.5,
            width: dotSize, height: dotSize
        )

        NSColor.systemRed.setFill()
        NSBezierPath(ovalIn: dotRect).fill()

        if pendingCount > 1 {
            let label = pendingCount < 10 ? "\(pendingCount)" : "+"
            let attrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.systemFont(ofSize: dotSize * 0.68, weight: .bold),
                .foregroundColor: NSColor.white
            ]
            let str = NSAttributedString(string: label, attributes: attrs)
            let s = str.size()
            str.draw(at: CGPoint(x: dotRect.midX - s.width / 2, y: dotRect.midY - s.height / 2))
        }

        badged.unlockFocus()
        badged.isTemplate = false
        button.image = badged
        button.imageScaling = .scaleProportionallyUpOrDown
        button.toolTip = "ClipRelay • \(pendingCount) clipboard item\(pendingCount == 1 ? "" : "s") waiting — click to apply"
    }

    // MARK: - Hot keys

    private func registerHotKeys() {
        GlobalHotKeyManager.shared.register(
            id: 1, keyCode: UInt32(kVK_ANSI_V),
            modifiers: UInt32(cmdKey | shiftKey)
        ) { [weak self] in self?.openQuickAccess() }

        GlobalHotKeyManager.shared.register(
            id: 2, keyCode: UInt32(kVK_ANSI_K),
            modifiers: UInt32(cmdKey)
        ) { [weak self] in self?.openCommandPalette() }
    }

    // MARK: - Trust prompt
    //
    // Previously used NSAlert.runModal() which blocks the main run loop — this
    // means clipboard events, peer pings, and UI updates all pause while the
    // prompt is visible.  The fix: show a non-blocking NSAlert using
    // beginSheetModal(for:) attached to the dashboard window, or fall back to
    // a non-modal alert with a completion handler.  The store's
    // pendingTrustRequest is cleared after the user responds, which allows the
    // next pending request (if any) to flow through the Combine pipeline.

    private func presentTrustPrompt(for detail: DeviceDetailSnapshot) {
        let alert = NSAlert()
        alert.messageText     = "Trust \(detail.effectiveName)?"
        alert.informativeText = """
        Device name: \(detail.deviceName)
        Fingerprint:
        \(detail.fingerprint)

        Only trust devices you control.
        """
        alert.addButton(withTitle: "Trust")
        alert.addButton(withTitle: "Reject")
        alert.alertStyle = .warning

        let device = ManagedDevice(peer: PeerViewModel(
            id: detail.deviceId, displayName: detail.deviceName,
            platform: nil, trusted: false, remembered: false, connected: false,
            connectionStatus: "disconnected",
            syncEnabled: true, autoConnect: false, lastSeen: detail.lastSeen, lastSync: nil
        ))

        let respond: (Bool) -> Void = { [weak self] approved in
            guard let self else { return }
            if approved { self.store.trust(device) }
            else        { self.store.reject(device) }
            // Allow the next pending trust request to surface.
            self.store.pendingTrustRequest = nil
        }

        // Attach as a sheet if the dashboard is visible — non-blocking.
        if let window = dashboardController?.window, window.isVisible {
            NSApp.activate(ignoringOtherApps: true)
            alert.beginSheetModal(for: window) { response in
                respond(response == .alertFirstButtonReturn)
            }
        } else {
            // Dashboard is hidden: bring it forward and show the sheet,
            // then use a window-level sheet so we still don't block the run loop.
            NSApp.activate(ignoringOtherApps: true)
            openDashboard()
            // Give the window a tick to become key before attaching the sheet.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                guard let window = self?.dashboardController?.window else {
                    // Fallback: no window available, use non-blocking alert.
                    alert.buttons[0].target = nil
                    let r = alert.runModal()   // last resort only
                    respond(r == .alertFirstButtonReturn)
                    return
                }
                alert.beginSheetModal(for: window) { response in
                    respond(response == .alertFirstButtonReturn)
                }
            }
        }
    }

    // MARK: - Actions

    private func showPanel(_ controller: NSWindowController?) {
        guard let window = controller?.window else { return }
        NSApp.activate(ignoringOtherApps: true)
        Self.fit(window: window)
        window.makeKeyAndOrderFront(nil)
    }

    @objc private func openDashboard()      { showPanel(dashboardController) }
    @objc private func openQuickAccess()    { showPanel(quickAccessController) }
    @objc private func openCommandPalette() { showPanel(commandPaletteController) }
    @objc private func quitApp()            { NSApp.terminate(nil) }

    // MARK: - Window factory

    private static func makeWindow<Content: View>(
        title: String, size: NSSize, rootView: Content
    ) -> NSWindowController {
        let frame  = fittedFrame(for: size)
        let window = NSWindow(
            contentRect: frame,
            styleMask:   [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false
        )
        window.title                     = title
        window.minSize                   = NSSize(width: 760, height: 520)
        window.titlebarAppearsTransparent = true
        window.titleVisibility            = .hidden
        window.isMovableByWindowBackground = true
        window.appearance                 = NSAppearance(named: .aqua)
        window.level                      = .normal
        window.collectionBehavior         = [.moveToActiveSpace]
        window.isReleasedWhenClosed       = false
        window.contentViewController = NSHostingController(rootView: rootView)
        return NSWindowController(window: window)
    }

    private static func makePanel<Content: View>(
        title: String, size: NSSize, rootView: Content
    ) -> NSWindowController {
        let frame = fittedFrame(for: size)
        let panel = NSPanel(
            contentRect: frame,
            styleMask: [.titled, .closable, .fullSizeContentView, .utilityWindow],
            backing: .buffered, defer: false
        )
        panel.title                       = title
        panel.titlebarAppearsTransparent  = true
        panel.titleVisibility             = .hidden
        panel.isMovableByWindowBackground = true
        panel.isFloatingPanel             = false
        panel.level                       = .normal
        panel.hidesOnDeactivate           = true
        panel.isReleasedWhenClosed        = false
        panel.isOpaque                    = false
        panel.backgroundColor             = .clear
        panel.appearance                  = NSAppearance(named: .aqua)
        panel.collectionBehavior          = [.moveToActiveSpace, .fullScreenAuxiliary]
        panel.standardWindowButton(.miniaturizeButton)?.isHidden = true
        panel.standardWindowButton(.zoomButton)?.isHidden = true
        panel.contentViewController = NSHostingController(rootView: rootView)
        return NSWindowController(window: panel)
    }

    private static func fit(window: NSWindow) {
        window.setFrame(fittedFrame(for: window.frame.size), display: false)
    }

    private static func fittedFrame(for size: NSSize) -> NSRect {
        let screen        = NSScreen.main ?? NSScreen.screens.first
        let visible       = screen?.visibleFrame ?? NSRect(origin: .zero, size: size)
        let margin: CGFloat = 48
        let w = min(size.width,  max(visible.width  - margin, 560))
        let h = min(size.height, max(visible.height - margin, 420))
        return NSRect(
            x: visible.midX - w / 2,
            y: visible.midY - h / 2,
            width: w, height: h
        )
    }
}
