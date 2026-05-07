import AppKit
import Carbon
import Combine
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let store = ClipRelayStore()
    private var statusItem: NSStatusItem!
    private let statusMenuItem = NSMenuItem(title: "Starting…", action: nil, keyEquivalent: "")
    private let lastSyncMenuItem = NSMenuItem(title: "Last sync: —", action: nil, keyEquivalent: "")
    private var dashboardController: NSWindowController?
    private var quickAccessController: NSWindowController?
    private var commandPaletteController: NSWindowController?
    private var preferencesController: NSWindowController?
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

    private func ensureSingleRunningInstance() -> Bool {
        guard let bundleIdentifier = Bundle.main.bundleIdentifier else {
            return true
        }

        let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundleIdentifier)
        guard running.count > 1 else {
            return true
        }

        let currentProcessIdentifier = ProcessInfo.processInfo.processIdentifier
        let existingInstance = running.first { $0.processIdentifier != currentProcessIdentifier }
        existingInstance?.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
        NSApp.terminate(nil)
        return false
    }

    private func setupMenuBar() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.toolTip = "ClipRelay"
        if let image = statusBarImage() {
            statusItem.button?.image = image
            statusItem.button?.imageScaling = .scaleProportionallyUpOrDown
            statusItem.button?.imagePosition = .imageOnly
            statusItem.button?.title = ""
        } else {
            statusItem.button?.title = "PB"
            statusItem.button?.font = .systemFont(ofSize: 12, weight: .semibold)
        }

        let menu = NSMenu()
        statusMenuItem.isEnabled = false
        lastSyncMenuItem.isEnabled = false
        menu.addItem(statusMenuItem)
        menu.addItem(lastSyncMenuItem)
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Open Dashboard", action: #selector(openDashboard), keyEquivalent: "0"))
        menu.addItem(NSMenuItem(title: "Quick Access", action: #selector(openQuickAccess), keyEquivalent: "v"))
        menu.addItem(NSMenuItem(title: "Command Palette", action: #selector(openCommandPalette), keyEquivalent: "k"))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Preferences…", action: #selector(openPreferences), keyEquivalent: ","))
        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit ClipRelay", action: #selector(quitApp), keyEquivalent: "q"))
        statusItem.menu = menu
    }

    private func statusBarImage() -> NSImage? {
        guard let url = Bundle.main.url(forResource: "StatusBarIcon", withExtension: "png"),
              let image = NSImage(contentsOf: url) else {
            return nil
        }
        image.size = NSSize(width: 18, height: 18)
        image.isTemplate = false
        return image
    }

    private func setupWindows() {
        dashboardController = Self.makeWindow(
            title: "ClipRelay",
            size: NSSize(width: 1160, height: 780),
            rootView: DashboardRootView(store: store)
        )
        quickAccessController = Self.makePanel(
            title: "Quick Access",
            size: NSSize(width: 460, height: 540),
            rootView: QuickAccessHistoryView(store: store)
        )
        commandPaletteController = Self.makePanel(
            title: "Command Palette",
            size: NSSize(width: 540, height: 320),
            rootView: CommandPaletteView(store: store)
        )
        preferencesController = Self.makeWindow(
            title: "Preferences",
            size: NSSize(width: 560, height: 700),
            rootView: PreferencesView(store: store)
        )
    }

    private func bindStore() {
        store.$connectionBanner
            .receive(on: RunLoop.main)
            .sink { [weak self] banner in
                self?.statusItem.button?.toolTip = banner
                self?.statusMenuItem.title = banner
            }
            .store(in: &cancellables)

        store.$status
            .receive(on: RunLoop.main)
            .sink { [weak self] status in
                let peerCount = status?.peerCount ?? 0
                self?.statusItem.button?.title = self?.statusItem.button?.image == nil ? (peerCount > 0 ? "PB \(peerCount)" : "PB") : ""
                self?.statusItem.button?.toolTip = peerCount > 0 ? "ClipRelay • \(peerCount) connected" : "ClipRelay"
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

    private func registerHotKeys() {
        GlobalHotKeyManager.shared.register(
            id: 1,
            keyCode: UInt32(kVK_ANSI_V),
            modifiers: UInt32(cmdKey | shiftKey)
        ) { [weak self] in
            self?.openQuickAccess()
        }

        GlobalHotKeyManager.shared.register(
            id: 2,
            keyCode: UInt32(kVK_ANSI_K),
            modifiers: UInt32(cmdKey)
        ) { [weak self] in
            self?.openCommandPalette()
        }
    }

    private func presentTrustPrompt(for detail: DeviceDetailSnapshot) {
        let alert = NSAlert()
        alert.messageText = "Trust \(detail.effectiveName)?"
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
        let device = ManagedDevice(
            id: detail.deviceId,
            name: detail.effectiveName,
            rawName: detail.deviceName,
            endpoint: nil,
            connectionState: .connecting,
            trustState: .untrusted,
            fingerprint: detail.fingerprint,
            lastSeen: detail.lastSeen,
            lastSync: nil,
            lastError: nil
        )

        if response == .alertFirstButtonReturn {
            store.trust(device)
        } else {
            store.reject(device)
        }
    }

    private func showPanel(_ controller: NSWindowController?) {
        guard let window = controller?.window else { return }
        NSApp.activate(ignoringOtherApps: true)
        window.center()
        window.makeKeyAndOrderFront(nil)
    }

    @objc private func openDashboard() {
        showPanel(dashboardController)
    }

    @objc private func openQuickAccess() {
        showPanel(quickAccessController)
    }

    @objc private func openCommandPalette() {
        showPanel(commandPaletteController)
    }

    @objc private func openPreferences() {
        showPanel(preferencesController)
    }

    @objc private func quitApp() {
        NSApp.terminate(nil)
    }

    private static func makeWindow<Content: View>(
        title: String,
        size: NSSize,
        rootView: Content
    ) -> NSWindowController {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.center()
        window.contentViewController = NSHostingController(rootView: rootView)
        return NSWindowController(window: window)
    }

    private static func makePanel<Content: View>(
        title: String,
        size: NSSize,
        rootView: Content
    ) -> NSWindowController {
        let panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.title = title
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.moveToActiveSpace]
        panel.contentViewController = NSHostingController(rootView: rootView)
        return NSWindowController(window: panel)
    }
}
