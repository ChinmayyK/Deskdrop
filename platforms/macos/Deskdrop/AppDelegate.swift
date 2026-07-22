import AppKit
import Carbon
import Combine
import SwiftUI
import UserNotifications
import Darwin

import Network

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    private let store = DeskdropStore()
    private var statusItem: NSStatusItem!
    private var menuBarDropView: MenuBarDropView?
    private var quickAccessWindow: NSWindow?
    private var diagnosticsWindow: NSWindow?
    private var fileBannerManager: FileBannerWindowManager!
    private var previousConnectedCount = 0
    private var menuPanel: NSPanel!
    private var localEventMonitor: Any?
    private var globalEventMonitor: Any?
    private var dashboardController:      NSWindowController?
    private var quickAccessController:    NSWindowController?
    private var commandPaletteController: NSWindowController?
    private var toastWindowManager:       DeskdropToastWindowManager?
    private var callBannerManager:         CallBannerWindowManager?
    private var cancellables = Set<AnyCancellable>()
    private var dropCanvasWindow: NSPanel?
    private var activityToken: NSObjectProtocol?
    private var dummyBrowser: NWBrowser?
    
    // Screenshot observing managed by ScreenshotObserver
    private var screenshotObserver: ScreenshotObserver?
    private var keyboardShortcutMonitor: Any?

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard ensureSingleRunningInstance() else { return }
        triggerLocalNetworkPrivacyPrompt()
        NSApp.setActivationPolicy(.accessory)
        // Restore user's theme preference (defaults to system)
        let savedTheme = UserDefaults.standard.string(forKey: "cr_app_theme") ?? "light"
        switch savedTheme {
        case "dark":   NSApp.appearance = NSAppearance(named: .darkAqua)
        case "light":  NSApp.appearance = NSAppearance(named: .aqua)
        default:       NSApp.appearance = nil
        }
        DaemonManager.shared.startDaemonIfNeeded()
        setupMainMenu()
        setupMenuBar()
        setupWindows()
        setupKeyboardShortcutMonitor()
        bindStore()
        toastWindowManager = DeskdropToastWindowManager(store: store)
        callBannerManager = CallBannerWindowManager(store: store)
        fileBannerManager = FileBannerWindowManager()
        registerHotKeys()
        registerSleepWakeObservers()
        registerStoreNotifications()
        // Request permission for system notifications (device-connected alerts)
        UNUserNotificationCenter.current().delegate = self
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
        store.start()
        startMacScreenshotObserver()
        
        // Prevent App Nap to ensure background daemon and network sync stay responsive
        activityToken = ProcessInfo.processInfo.beginActivity(options: [.userInitiated], reason: "Deskdrop Background Sync")
    }

    /// Observe notifications posted by DeskdropStore so it stays decoupled from AppKit.
    private func registerStoreNotifications() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(openQuickAccess),
            name: .deskdropOpenHistoryPanel,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(openCommandPalette),
            name: .deskdropOpenCommandPalette,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(closeDropCanvas),
            name: .init("closeDropCanvas"),
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(ensureDaemonResponsiveFromStore),
            name: .deskdropEnsureDaemon,
            object: nil
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let token = activityToken {
            ProcessInfo.processInfo.endActivity(token)
            activityToken = nil
        }
        store.stop()
        DaemonManager.shared.terminate()
    }

    @objc private func ensureDaemonResponsiveFromStore() {
        DaemonManager.shared.ensureDaemonResponsiveFromStore()
    }

    private func openDiagnostics() {
        if diagnosticsWindow == nil {
            let win = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 400, height: 350),
                styleMask: [.titled, .closable, .fullSizeContentView],
                backing: .buffered, defer: false
            )
            win.titleVisibility = .hidden
            win.titlebarAppearsTransparent = true
            win.isMovableByWindowBackground = true
            win.center()
            win.contentViewController = NSHostingController(rootView: DiagnosticsView(store: store))
            win.level = .floating
            win.isReleasedWhenClosed = false
            diagnosticsWindow = win
        }
        diagnosticsWindow?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    // MARK: - Single instance guard

    private func triggerLocalNetworkPrivacyPrompt() {
        let params = NWParameters()
        params.includePeerToPeer = true
        dummyBrowser = NWBrowser(for: .bonjour(type: "_deskdrop._tcp", domain: "local."), using: params)
        dummyBrowser?.stateUpdateHandler = { _ in }
        dummyBrowser?.start(queue: .main)
        // We do not cancel the dummy browser immediately because macOS
        // sometimes drops the prompt if the browser is cancelled too quickly.
    }

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
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        // ── Replace the default button with a custom drag-and-drop view ──────────
        if let button = statusItem.button {
            // Do not register drag types on the button itself, otherwise it intercepts
            // the drag events and prevents our custom MenuBarDropView from receiving them.

            button.image = statusBarImage()
            button.imagePosition = .imageOnly
            button.target = self
            button.action = #selector(menuBarClicked)

            let dropView = MenuBarDropView(frame: button.bounds)
            dropView.autoresizingMask = [.width, .height]
            dropView.delegate  = self
            button.addSubview(dropView)
            menuBarDropView = dropView
            button.toolTip  = "Deskdrop — Drag files here to send to your device"
            
            button.window?.registerForDraggedTypes([
                .fileURL,
                .init(rawValue: "com.apple.pasteboard.promised-file-url"),
                .init(rawValue: "com.apple.NSFilePromiseItemMetaData")
            ])
            button.window?.delegate = self
        }

        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 320, height: 350),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .popUpMenu
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.contentViewController = NSHostingController(rootView: MenuBarPopoverView(store: store, onAction: { [weak self] action in
            self?.handlePopoverAction(action)
        }))
        menuPanel = panel
    }

    private func handlePopoverAction(_ action: MenuBarPopoverAction) {
        closeMenuPanel()
        switch action {
        case .dashboard: openDashboard()
        case .quickAccess: openQuickAccess()
        case .commandPalette: openCommandPalette()
        case .pushClipboard: forcePushClipboard()
        case .sendFile: sendFileFromMenu()
        case .scan: scanDevices()
        case .diagnostics: openDiagnostics()
        case .quit: quitApp()
        }
    }

    // MARK: - Main Menu & Keyboard Shortcuts

    private func setupMainMenu() {
        let mainMenu = NSMenu()

        // 1. App Menu
        let appMenuItem = NSMenuItem()
        mainMenu.addItem(appMenuItem)
        let appMenu = NSMenu()
        appMenuItem.submenu = appMenu
        appMenu.addItem(withTitle: "About Deskdrop", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Preferences...", action: #selector(openPreferencesFromMenu), keyEquivalent: ",")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Hide Deskdrop", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = appMenu.addItem(withTitle: "Hide Others", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        appMenu.addItem(withTitle: "Show All", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Quit Deskdrop", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        // 2. File Menu
        let fileMenuItem = NSMenuItem()
        mainMenu.addItem(fileMenuItem)
        let fileMenu = NSMenu(title: "File")
        fileMenuItem.submenu = fileMenu
        fileMenu.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")

        // 3. Edit Menu (Enables standard Cmd+C/V/X/A/Z in text controls)
        let editMenuItem = NSMenuItem()
        mainMenu.addItem(editMenuItem)
        let editMenu = NSMenu(title: "Edit")
        editMenuItem.submenu = editMenu
        editMenu.addItem(withTitle: "Undo", action: NSSelectorFromString("undo:"), keyEquivalent: "z")
        editMenu.addItem(withTitle: "Redo", action: NSSelectorFromString("redo:"), keyEquivalent: "Z")
        editMenu.addItem(NSMenuItem.separator())
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")

        // 4. Window Menu
        let windowMenuItem = NSMenuItem()
        mainMenu.addItem(windowMenuItem)
        let windowMenu = NSMenu(title: "Window")
        windowMenuItem.submenu = windowMenu
        windowMenu.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")

        NSApp.mainMenu = mainMenu
    }

    @objc private func openPreferencesFromMenu() {
        showPanel(dashboardController)
    }

    private func setupKeyboardShortcutMonitor() {
        keyboardShortcutMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            guard flags.contains(.command), !flags.contains(.control), !flags.contains(.option) else {
                return event
            }
            guard let char = event.charactersIgnoringModifiers?.lowercased() else {
                return event
            }
            switch char {
            case "w":
                if let win = NSApp.keyWindow {
                    win.performClose(nil)
                    if win.isVisible { win.close() }
                    return nil
                }
            case "m":
                if let win = NSApp.keyWindow {
                    win.performMiniaturize(nil)
                    if !win.isMiniaturized { win.miniaturize(nil) }
                    return nil
                }
            case "q":
                NSApp.terminate(nil)
                return nil
            default:
                break
            }
            return event
        }
    }

    // MARK: - Menu Bar Icon (Original Backup for Rollback)
    private func statusBarImageOriginal() -> NSImage? {
        let size = NSSize(width: 16, height: 16)
        let image = NSImage(size: size)
        
        guard let symbol = NSImage(systemSymbolName: "arrow.down.doc.fill", accessibilityDescription: "Deskdrop") else { return nil }
        
        let config = NSImage.SymbolConfiguration(pointSize: 14, weight: .regular)
        let configuredSymbol = symbol.withSymbolConfiguration(config) ?? symbol
        
        image.lockFocus()
        
        let symbolSize = configuredSymbol.size
        let x = (size.width - symbolSize.width) / 2.0
        let y = (size.height - symbolSize.height) / 2.0
        let rect = NSRect(x: x, y: y, width: symbolSize.width, height: symbolSize.height)
        
        configuredSymbol.draw(in: rect)
        image.unlockFocus()
        
        image.isTemplate = true
        return image
    }

    private func statusBarImage() -> NSImage? {
        // Experimental: Connected Devices icon (Mac + iPhone)
        let size = NSSize(width: 18, height: 16)
        let image = NSImage(size: size)
        
        guard let symbol = NSImage(systemSymbolName: "laptopcomputer.and.iphone", accessibilityDescription: "Deskdrop") else {
            return statusBarImageOriginal()
        }
        
        let config = NSImage.SymbolConfiguration(pointSize: 13, weight: .medium)
        let configuredSymbol = symbol.withSymbolConfiguration(config) ?? symbol
        
        image.lockFocus()
        
        let symbolSize = configuredSymbol.size
        let x = (size.width - symbolSize.width) / 2.0
        let y = (size.height - symbolSize.height) / 2.0
        let rect = NSRect(x: x, y: y, width: symbolSize.width, height: symbolSize.height)
        
        configuredSymbol.draw(in: rect)
        image.unlockFocus()
        
        image.isTemplate = true
        return image
    }

    // MARK: - Windows

    private func setupWindows() {
        dashboardController = Self.makeWindow(
            title: "Deskdrop",
            size:  NSSize(width: 1200, height: 760),
            rootView: RootContainerView(store: store)
        )
        quickAccessController = Self.makePanel(
            title: "Quick Access",
            size:  NSSize(width: 680, height: 540),
            rootView: QuickAccessHistoryView(store: store)
        )
        commandPaletteController = Self.makePanel(
            title: "Command Palette",
            size:  NSSize(width: 520, height: 400),
            rootView: CommandPaletteView(store: store)
        )
        EdgeDropWindowManager.shared.setup(with: store)
    }

    // MARK: - Store bindings

    private func bindStore() {
        store.$statusLine
            .receive(on: RunLoop.main)
            .sink { [weak self] banner in
                self?.statusItem.button?.toolTip = "Deskdrop • \(banner)"
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

        // ── Instant "device connected" notification ──────────────────────────
        store.$peers
            .receive(on: RunLoop.main)
            .map { $0.map(ManagedDevice.init).filter { $0.isConnected && $0.trustState == .trusted } }
            .sink { [weak self] (devices: [ManagedDevice]) in
                guard let self else { return }
                let count = devices.count
                if count > self.previousConnectedCount, let device = devices.last ?? devices.first {
                    // Fire immediately — no delay
                    self.store.showToast(
                        title: "\(device.name) connected",
                        body: "Ready to sync.",
                        tint: CRTheme.accentGreen,
                        systemImage: "link.badge.plus",
                        ttl: 4.0
                    )
                    // Animate the menu bar icon briefly
                    self.pulseMenuBarIcon()
                } else if count == 0 && self.previousConnectedCount > 0 {
                    self.store.showToast(
                        title: "Device disconnected",
                        body: "No devices currently connected.",
                        tint: CRTheme.inkSoft,
                        systemImage: "wifi.slash",
                        ttl: 3.0
                    )
                }
                self.previousConnectedCount = count
            }
            .store(in: &cancellables)
    }

    // MARK: - System notifications

    private func sendSystemNotification(title: String, body: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body  = body
        content.sound = .default
        
        let req = UNNotificationRequest(
            identifier: UUID().uuidString,
            content: content,
            trigger: nil
        )
        UNUserNotificationCenter.current().add(req)
    }

    // Allow notifications to show as banners even when app is in foreground
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification, withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        completionHandler([.banner, .sound])
    }
    
    // Handle notification click
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse, withCompletionHandler completionHandler: @escaping () -> Void) {
        Task { @MainActor [weak self] in
            NSApp.activate(ignoringOtherApps: true)
            self?.openQuickAccess()
            completionHandler()
        }
    }

    private func pulseMenuBarIcon() {
        guard let button = statusItem.button else { return }
        let anim = CABasicAnimation(keyPath: "opacity")
        anim.fromValue  = 1.0
        anim.toValue    = 0.3
        anim.duration   = 0.18
        anim.autoreverses = true
        anim.repeatCount  = 3
        button.layer?.add(anim, forKey: "pulse")
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
            
        NotificationCenter.default.addObserver(self,
            selector: #selector(handleActivityReceived(_:)),
            name: NSNotification.Name("deskdropActivityReceived"),
            object: nil)

        wsnc.addObserver(self,
            selector: #selector(handleSystemSleep),
            name: NSWorkspace.willSleepNotification,
            object: nil)
    }

    @objc private func handleActivityReceived(_ notification: Notification) {
        guard let entry = notification.object as? IpcActivityEntry else { return }
        
        switch entry.kind {
        case "remote_clipboard_available":
            let title: String
            let body: String
            
            if entry.applied_locally {
                title = "Clipboard Received"
                body = "Copied from \(entry.device_name)"
            } else {
                title = "Clipboard Received"
                body = "From \(entry.device_name) (Click to apply)"
            }

            
            if !entry.applied_locally {
                store.showToast(
                    title: title,
                    body: body,
                    tint: CRTheme.accentGreen,
                    systemImage: "doc.on.clipboard",
                    ttl: 6.0,
                    primaryAction: ToastAction(title: "Apply", role: .primary) { [weak self] in
                        Task { @MainActor [weak self] in
                            await self?.store.applyClipboard(entry: entry)
                        }
                    }
                )
            } else {
                store.showToast(
                    title: title,
                    body: body,
                    tint: CRTheme.accentGreen,
                    systemImage: "doc.on.clipboard",
                    ttl: 4.0
                )
            }
            
        case "file_transfer_complete":
            // Only notify for incoming files where dest_path is populated
            guard let destPath = entry.dest_path else { return }
            
            let fileName = entry.file_name ?? URL(fileURLWithPath: destPath).lastPathComponent
            let isScreenshot = fileName.lowercased().contains("screenshot")

            if isScreenshot {
                let currentUrl = URL(fileURLWithPath: destPath)
                let downloadsDir = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first!
                let deskdropDir = downloadsDir.appendingPathComponent("Deskdrop")
                let screenshotsDir = deskdropDir.appendingPathComponent("android_screenshot")
                
                try? FileManager.default.createDirectory(at: screenshotsDir, withIntermediateDirectories: true)
                let newDestUrl = screenshotsDir.appendingPathComponent(fileName)
                
                do {
                    if FileManager.default.fileExists(atPath: newDestUrl.path) {
                        try FileManager.default.removeItem(at: newDestUrl)
                    }
                    try FileManager.default.moveItem(at: currentUrl, to: newDestUrl)
                    
                    if let image = NSImage(contentsOf: newDestUrl) {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.writeObjects([image])
                    }
                    
                    store.showToast(
                        title: "Screenshot Synced",
                        body: "Copied to clipboard",
                        tint: CRTheme.accentBlue,
                        systemImage: "camera.viewfinder",
                        ttl: 4.0,
                        primaryAction: ToastAction(title: "Reveal", role: .primary) {
                            NSWorkspace.shared.activateFileViewerSelecting([newDestUrl])
                        }
                    )
                    return // Skip the standard notification
                } catch {
                    print("Failed to handle screenshot sync: \(error)")
                }
            }
            
            let title = "File Received"
            let body = fileName
            
            fileBannerManager.show(title: title, body: body)
            sendSystemNotification(title: title, body: body)
            
        case "remote_notification":
            // Respect the user's toggle for Android Notification Mirroring
            let mirrorEnabled = UserDefaults.standard.object(forKey: "mirrorAndroidNotifications") as? Bool ?? true
            guard mirrorEnabled else { return }

            let title = entry.summary.components(separatedBy: ": ").first ?? "Notification"
            let body = entry.summary.components(separatedBy: ": ").dropFirst().joined(separator: ": ")
            
            // Send native macOS notification (which also plays sound based on OS settings)
            sendSystemNotification(title: title, body: body.isEmpty ? entry.summary : body)

        default:
            // Ignore other events like local copies, device connections/disconnections, etc.
            break
        }
    }


    @objc private func handleSystemWake() {
        Task { @MainActor [weak self] in
            guard let self else { return }
            NSLog("Deskdrop: system woke — starting reconnect sequence")

            // Stage 1: Immediate refresh + discovery rescan
            await store.refresh()
            store.scanForDevices()

            // Stage 2: Exponential retry — stop early if peers reconnect
            let retryDelays: [UInt64] = [2_000_000_000, 5_000_000_000]
            for delay in retryDelays {
                if store.connectedCount > 0 { break }
                try? await Task.sleep(nanoseconds: delay)
                await store.refresh()
                store.scanForDevices()
            }

            // Announce reconnection result
            if let peer = store.connectedDevices.first {
                store.showToast(
                    title: "Connected to \(peer.name)",
                    body: "Clipboard and files synchronized.",
                    tint: CRTheme.accentGreen,
                    systemImage: "link.badge.plus",
                    ttl: 3.0
                )
            } else {
                NSLog("Deskdrop: wake reconnect — no peers found after retries")
            }
        }
    }

    @objc private func handleSystemSleep() {
        // Nothing to do — the Rust engine handles clean peer shutdown on sleep.
        // We just log for diagnosability.
        NSLog("Deskdrop: system going to sleep")
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
            button.toolTip = "Deskdrop"
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
        button.toolTip = "Deskdrop • \(pendingCount) clipboard item\(pendingCount == 1 ? "" : "s") waiting — click to apply"
        menuBarDropView?.badgeCount = pendingCount
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

        // F24: ⌘⇧C — Force push current clipboard to all connected peers
        GlobalHotKeyManager.shared.register(
            id: 3, keyCode: UInt32(kVK_ANSI_C),
            modifiers: UInt32(cmdKey | shiftKey)
        ) { [weak self] in self?.forcePushClipboard() }

        // Ctrl+D to toggle Drop Canvas in lower middle
        GlobalHotKeyManager.shared.register(
            id: 4, keyCode: UInt32(kVK_ANSI_D),
            modifiers: UInt32(controlKey)
        ) { [weak self] in self?.toggleDropCanvas() }
    }

    /// F24: Push the current Mac clipboard to all connected peers immediately.
    private func forcePushClipboard() {
        guard store.connectedCount > 0 else {
            store.showToast(
                title: "No Devices Connected",
                body: "Connect a device to push clipboard.",
                tint: CRTheme.inkSoft,
                systemImage: "wifi.slash",
                ttl: 2.5
            )
            return
        }
        Task {
            do {
                try await DeskdropIPCClient.shared.sendClipboardCurrent(targetDeviceId: store.defaultTargetDevice?.id)
                store.showToast(
                    title: "Clipboard Synced",
                    body: "Pushed to all connected devices.",
                    tint: CRTheme.accentGreen,
                    systemImage: "arrow.up.circle.fill",
                    ttl: 2.0
                )
            } catch {
                store.showToast(
                    title: "Sync Failed",
                    body: error.localizedDescription,
                    tint: Color.red,
                    systemImage: "exclamationmark.triangle",
                    ttl: 3.0
                )
            }
        }
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
            syncEnabled: true, autoConnect: false, lastError: nil,
            pairingRequested: false, outgoingPairingWaiting: false, pairingPin: nil,
            explicitDisconnect: false,
            lastSeen: detail.lastSeen, lastDiscoveryAt: nil, lastSync: nil, ip: nil
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
    @objc private func scanDevices()        { store.scanForDevices() }

    @objc private func sendFileFromMenu() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseFiles          = true
        panel.canChooseDirectories    = false
        panel.prompt                  = "Send"
        panel.message                 = "Choose files to send to connected devices"
        if panel.runModal() == .OK, !panel.urls.isEmpty {
            store.sendFiles(urls: panel.urls, toPeer: store.defaultTargetDevice?.id)
        }
    }

    @objc private func pushClipboardFromMenu() {
        forcePushClipboard()
    }

    /// F22: Grab the active URL from the frontmost browser and push it to the connected device.
    @objc private func sendBrowserUrlToDevice() {
        guard store.connectedCount > 0 else {
            store.showToast(
                title: "No Devices Connected",
                body: "Connect a device first.",
                tint: CRTheme.inkSoft,
                systemImage: "wifi.slash",
                ttl: 2.5
            )
            return
        }

        // Try multiple browsers: Safari, Chrome, Arc, Brave, Edge
        let scripts: [(String, String)] = [
            ("Safari",            "tell application \"Safari\" to get URL of front document"),
            ("Google Chrome",     "tell application \"Google Chrome\" to get URL of active tab of front window"),
            ("Arc",               "tell application \"Arc\" to get URL of active tab of front window"),
            ("Brave Browser",     "tell application \"Brave Browser\" to get URL of active tab of front window"),
            ("Microsoft Edge",    "tell application \"Microsoft Edge\" to get URL of active tab of front window"),
        ]

        var foundUrl: String?
        let workspace = NSWorkspace.shared
        for (appName, script) in scripts {
            // Only try browsers that are actually running
            if workspace.runningApplications.contains(where: {
                $0.localizedName == appName && $0.isActive
            }) || workspace.frontmostApplication?.localizedName == appName {
                if let appleScript = NSAppleScript(source: script) {
                    var error: NSDictionary?
                    let result = appleScript.executeAndReturnError(&error)
                    if error == nil, let url = result.stringValue, url.hasPrefix("http") {
                        foundUrl = url
                        break
                    }
                }
            }
        }

        // Fallback: check if the clipboard already contains a URL
        if foundUrl == nil {
            if let clipboardText = NSPasteboard.general.string(forType: .string),
               clipboardText.hasPrefix("http://") || clipboardText.hasPrefix("https://") {
                foundUrl = clipboardText
            }
        }

        guard let url = foundUrl else {
            store.showToast(
                title: "No URL Found",
                body: "Open a browser tab with a URL, or copy a URL to your clipboard.",
                tint: CRTheme.inkSoft,
                systemImage: "safari",
                ttl: 3.0
            )
            return
        }

        // Set the URL on the clipboard and push it
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(url, forType: .string)

        Task {
            do {
                try await DeskdropIPCClient.shared.sendClipboardCurrent(targetDeviceId: store.defaultTargetDevice?.id)
                store.showToast(
                    title: "URL Sent",
                    body: url,
                    tint: CRTheme.accentBlue,
                    systemImage: "link.circle.fill",
                    ttl: 2.5
                )
            } catch {
                store.showToast(
                    title: "Send Failed",
                    body: error.localizedDescription,
                    tint: Color.red,
                    systemImage: "exclamationmark.triangle",
                    ttl: 3.0
                )
            }
        }
    }

    @objc private func connectManually() {
        let alert = NSAlert()
        alert.messageText     = "Connect to Device by IP"
        alert.informativeText = "Enter the IP address of the device running Deskdrop.\nMac IP: \(Self.localWiFiIP() ?? "unknown")"
        alert.addButton(withTitle: "Connect")
        alert.addButton(withTitle: "Cancel")

        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 22))
        input.placeholderString = "192.168.x.x"
        input.bezelStyle        = .roundedBezel
        alert.accessoryView     = input
        alert.window.initialFirstResponder = input

        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let host = input.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !host.isEmpty else { return }
        store.connectManual(host: host)
    }

    private static func localWiFiIP() -> String? {
        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0 else { return nil }
        defer { freeifaddrs(ifaddr) }
        var ptr = ifaddr
        while let addr = ptr {
            let fa   = addr.pointee
            let name = String(cString: fa.ifa_name)
            if fa.ifa_addr.pointee.sa_family == UInt8(AF_INET),
               name.hasPrefix("en") {
                var buf = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
                var sin = fa.ifa_addr.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { $0.pointee }
                inet_ntop(AF_INET, &sin.sin_addr, &buf, socklen_t(INET_ADDRSTRLEN))
                return String(cString: buf)
            }
            ptr = fa.ifa_next
        }
        return nil
    }

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
        window.minSize                   = NSSize(width: 900, height: 600)
        window.titlebarAppearsTransparent = true
        window.titleVisibility            = .hidden
        window.isMovableByWindowBackground = true
        window.level                      = .normal
        window.collectionBehavior         = [.moveToActiveSpace]
        window.isReleasedWhenClosed       = false
        window.backgroundColor            = .clear
        window.isOpaque                   = false
        window.hasShadow                  = false
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

// MARK: - MenuBarDropViewDelegate

extension AppDelegate: MenuBarDropViewDelegate {
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        let urls = filenames.map { URL(fileURLWithPath: $0) }
        store.sendFiles(urls: urls, toPeer: store.defaultTargetDevice?.id)
    }

    func menuBarDropView(_ view: MenuBarDropView, didReceiveFiles urls: [URL]) {
        store.sendFiles(urls: urls, toPeer: store.defaultTargetDevice?.id)
        // Brief visual feedback
        store.showToast(
            title: "Sending \(urls.count) file\(urls.count == 1 ? "" : "s")",
            body: urls.map(\.lastPathComponent).joined(separator: ", "),
            tint: CRTheme.brandElectric,
            systemImage: "arrow.up.doc.fill",
            ttl: 3.5
        )
    }

    @objc private func menuBarClicked() {
        guard let button = statusItem.button, let window = button.window else { return }
        
        if menuPanel.isVisible {
            closeMenuPanel()
        } else {
            // The button's window frame is already in screen coordinates
            let screenRect = window.frame
            
            let panelWidth: CGFloat = 320
            let panelHeight: CGFloat = 350
            var x = screenRect.midX - (panelWidth / 2)
            
            // Keep panel fully on-screen (prevent overflowing off right or left edge)
            let screen = window.screen ?? NSScreen.main
            if let visibleFrame = screen?.visibleFrame {
                let padding: CGFloat = 12
                if x + panelWidth > visibleFrame.maxX - padding {
                    x = visibleFrame.maxX - panelWidth - padding
                }
                if x < visibleFrame.minX + padding {
                    x = visibleFrame.minX + padding
                }
            }
            
            // Anchor it flush with the bottom of the menu bar
            let y = screenRect.minY - panelHeight - 2
            
            menuPanel.setFrame(NSRect(x: x, y: y, width: panelWidth, height: panelHeight), display: true)
            menuPanel.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            
            // Monitor clicks outside to close
            localEventMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
                if event.window != self?.menuPanel {
                    self?.closeMenuPanel()
                }
                return event
            }
            globalEventMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
                self?.closeMenuPanel()
            }
        }
    }

    private func closeMenuPanel() {
        menuPanel.orderOut(nil)
        if let local = localEventMonitor { NSEvent.removeMonitor(local); localEventMonitor = nil }
        if let global = globalEventMonitor { NSEvent.removeMonitor(global); globalEventMonitor = nil }
    }

    private func ensureDropCanvasWindow() {
        if dropCanvasWindow == nil {
            let panel = NSPanel(
                contentRect: NSRect(x: 0, y: 0, width: 320, height: 216),
                styleMask: [.borderless, .nonactivatingPanel],
                backing: .buffered,
                defer: false
            )
            panel.isOpaque = false
            panel.backgroundColor = .clear
            panel.hasShadow = true
            panel.level = .popUpMenu
            panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
            panel.contentViewController = NSHostingController(rootView: DropCanvasView(store: store))
            dropCanvasWindow = panel
        }
    }

    @objc private func toggleDropCanvas() {
        if let win = dropCanvasWindow, win.isVisible {
            closeDropCanvas()
            return
        }
        ensureDropCanvasWindow()
        guard let screen = NSScreen.main else { return }
        
        let panelWidth: CGFloat = 320
        let panelHeight: CGFloat = 216
        let x = screen.frame.midX - (panelWidth / 2)
        // Lower middle of screen
        let y = screen.visibleFrame.minY + 80
        
        dropCanvasWindow?.setFrame(NSRect(x: x, y: y, width: panelWidth, height: panelHeight), display: true)
        dropCanvasWindow?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func menuBarDropViewDidEnterDrag(_ view: MenuBarDropView) {
        ensureDropCanvasWindow()
        
        guard let button = statusItem.button, let window = button.window else { return }
        
        let buttonRect = button.convert(button.bounds, to: nil)
        let buttonScreenRect = window.convertToScreen(buttonRect)
        
        let panelWidth: CGFloat = 320
        let panelHeight: CGFloat = 216
        var x = buttonScreenRect.midX - (panelWidth / 2)
        
        let screen = window.screen ?? NSScreen.main
        if let visibleFrame = screen?.visibleFrame {
            let padding: CGFloat = 12
            if x + panelWidth > visibleFrame.maxX - padding {
                x = visibleFrame.maxX - panelWidth - padding
            }
            if x < visibleFrame.minX + padding {
                x = visibleFrame.minX + padding
            }
        }
        
        let y = buttonScreenRect.minY - panelHeight - 8
        
        dropCanvasWindow?.setFrame(NSRect(x: x, y: y, width: panelWidth, height: panelHeight), display: true)
        dropCanvasWindow?.orderFrontRegardless()
    }

    func menuBarDropViewDidExitDrag(_ view: MenuBarDropView) {
        // Wait briefly. If the mouse entered the window, don't close.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            guard let self = self, let win = self.dropCanvasWindow else { return }
            let mouseLoc = NSEvent.mouseLocation
            if win.frame.contains(mouseLoc) {
                // Drag moved into the window, leave it open!
                return
            }
            self.closeDropCanvas()
        }
    }

    @objc private func closeDropCanvas() {
        dropCanvasWindow?.orderOut(nil)
    }

    // MARK: - Screenshot Sync

    private func startMacScreenshotObserver() {
        screenshotObserver = ScreenshotObserver(onScreenshot: { [weak self] url in
            self?.handleNewScreenshot(url: url)
        })
        screenshotObserver?.start()
    }
    
    private func handleNewScreenshot(url: URL) {
        let isEnabled = UserDefaults.standard.object(forKey: "autoForwardMacScreenshots") as? Bool ?? false
        guard isEnabled else { return }
        
        NSLog("Deskdrop: Auto-syncing Mac screenshot -> Android: \(url.path)")
        Task { @MainActor [weak self] in
            self?.store.sendFile(url: url)
        }
    }
}

// MARK: - NSDraggingDestination for Status Bar Window
extension AppDelegate: NSDraggingDestination, NSWindowDelegate {
    func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        return menuBarDropView?.draggingEntered(sender) ?? []
    }
    func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        return menuBarDropView?.draggingUpdated(sender) ?? []
    }
    func draggingExited(_ sender: NSDraggingInfo?) {
        menuBarDropView?.draggingExited(sender)
    }
    func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        return menuBarDropView?.performDragOperation(sender) ?? false
    }
}
