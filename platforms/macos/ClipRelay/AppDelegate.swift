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

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard ensureSingleRunningInstance() else { return }
        NSApp.setActivationPolicy(.accessory)
        setupMenuBar()
        setupWindows()
        bindStore()
        registerHotKeys()
        store.start()
        openDashboard()
    }

    func applicationWillTerminate(_ notification: Notification) {
        store.stop()
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
        store.$connectionBanner
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

        store.$quickSendContext
            .dropFirst()
            .sink { [weak self] context in
                guard context != nil else { return }
                self?.showPanel(self?.quickAccessController)
            }
            .store(in: &cancellables)

        store.$pendingTrustRequest
            .compactMap { $0 }
            .sink { [weak self] detail in
                self?.presentTrustPrompt(for: detail)
            }
            .store(in: &cancellables)
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

    private func presentTrustPrompt(for detail: DeviceDetailSnapshot) {
        let alert = NSAlert()
        alert.messageText    = "Trust \(detail.effectiveName)?"
        alert.informativeText = """
        Device name: \(detail.deviceName)
        Fingerprint:
        \(detail.fingerprint)

        Only trust devices you control.
        """
        alert.addButton(withTitle: "Trust")
        alert.addButton(withTitle: "Reject")
        alert.alertStyle = .warning
        NSApp.activate(ignoringOtherApps: true)
        let response = alert.runModal()
        let device   = ManagedDevice(peer: PeerViewModel(
            id: detail.deviceId, displayName: detail.deviceName,
            platform: nil, trusted: false, remembered: false, connected: false,
            syncEnabled: true, autoConnect: false, lastSeen: detail.lastSeen, lastSync: nil
        ))
        if response == .alertFirstButtonReturn { store.trust(device) }
        else { store.reject(device) }
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
        window.contentViewController = NSHostingController(rootView: rootView)
        return NSWindowController(window: window)
    }

    private static func makePanel<Content: View>(
        title: String, size: NSSize, rootView: Content
    ) -> NSWindowController {
        let frame = fittedFrame(for: size)
        let panel = NSPanel(
            contentRect: frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered, defer: false
        )
        panel.title              = title
        panel.isFloatingPanel    = true
        panel.level              = .floating
        panel.isOpaque           = false
        panel.backgroundColor    = .clear
        panel.collectionBehavior = [.moveToActiveSpace]
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
