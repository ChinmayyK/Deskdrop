import Foundation

@MainActor
final class DaemonManager {
    static let shared = DaemonManager()
    
    private var daemonProcess: Process?

    private init() {}

    func startDaemonIfNeeded() {
        if isDaemonSocketPresent() {
            ensureDaemonResponsive(forceRestartOnFailure: true)
            return
        }
        launchDaemonProcess()
    }

    private func launchDaemonProcess() {
        cleanupDaemonSocketIfNeeded()

        let candidates = [
            Bundle.main.resourceURL?.appendingPathComponent("deskdrop-daemon"),
            Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("deskdrop-daemon"),
            URL(fileURLWithPath: "/usr/local/bin/deskdrop-daemon"),
            URL(fileURLWithPath: "/opt/homebrew/bin/deskdrop-daemon")
        ].compactMap { $0 }

        guard let daemonURL = candidates.first(where: {
            FileManager.default.isExecutableFile(atPath: $0.path)
        }) else {
            NSLog("Deskdrop: deskdrop-daemon not found in bundle or PATH candidates")
            return
        }

        let process = Process()
        process.executableURL = daemonURL
        process.environment = ProcessInfo.processInfo.environment.merging([
            "DESKDROP_LOG": "info"
        ]) { current, _ in current }

        do {
            try process.run()
            daemonProcess = process
            NSLog("Deskdrop: started daemon at \(daemonURL.path)")
        } catch {
            NSLog("Deskdrop: failed to start daemon: \(error.localizedDescription)")
        }
    }

    func ensureDaemonResponsiveFromStore() {
        ensureDaemonResponsive(forceRestartOnFailure: true)
    }

    private func ensureDaemonResponsive(forceRestartOnFailure: Bool) {
        Task {
            let responded = await withTaskGroup(of: Bool.self) { group in
                group.addTask {
                    do {
                        try await DeskdropIPCClient.shared.ping()
                        return true
                    } catch {
                        return false
                    }
                }
                group.addTask {
                    try? await Task.sleep(nanoseconds: 1_000_000_000)
                    return false
                }
                let result = await group.next() ?? false
                group.cancelAll()
                return result
            }
            if !responded && forceRestartOnFailure {
                self.daemonProcess?.terminate()
                self.daemonProcess = nil
                self.cleanupDaemonSocketIfNeeded()
                self.launchDaemonProcess()
            }
        }
    }

    private func candidateSocketPaths() -> [String] {
        var paths: [String] = []
        if let runtime = ProcessInfo.processInfo.environment["XDG_RUNTIME_DIR"] {
            paths.append("\(runtime)/deskdrop.sock")
        }
        paths.append("/tmp/deskdrop-\(getuid())/deskdrop.sock")
        if let home = ProcessInfo.processInfo.environment["HOME"] {
            paths.append("\(home)/.deskdrop.sock")
        }
        return paths
    }

    private func isDaemonSocketPresent() -> Bool {
        for path in candidateSocketPaths() {
            if FileManager.default.fileExists(atPath: path) {
                return true
            }
        }
        return false
    }

    private func cleanupDaemonSocketIfNeeded() {
        for path in candidateSocketPaths() {
            if FileManager.default.fileExists(atPath: path) {
                try? FileManager.default.removeItem(atPath: path)
            }
        }
    }
    
    func terminate() {
        daemonProcess?.terminate()
        daemonProcess = nil
    }
}
