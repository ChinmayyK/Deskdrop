import SwiftUI

struct PreferencesView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var workingCopy = ClipRelaySettingsSnapshot(
        port: 47823,
        deviceName: "",
        syncEnabled: true,
        syncText: true,
        syncImages: true,
        syncFiles: true,
        syncMode: .auto,
        maxPayloadBytes: 64 * 1024 * 1024,
        historyLimit: 50,
        maxHistoryTextBytes: 64 * 1024,
        showReceiveNotification: true,
        requireTofuConfirmation: true,
        blockedDeviceIds: [],
        blockSensitiveText: true,
        ignorePatterns: [],
        clipboardPollMs: 100,
        maxPushesPerSec: 10,
        rateLimitBurst: 3,
        smartSyncDuplicateWindowMs: 1500,
        smartSyncDebounceMs: 150,
        startOnLogin: false
    )

    @State private var ignorePatternInput = ""

    var body: some View {
        Form {
            Section("General") {
                TextField("Device name", text: $workingCopy.deviceName)
                Toggle("Enable syncing", isOn: $workingCopy.syncEnabled)
                Picker("Sync mode", selection: $workingCopy.syncMode) {
                    ForEach(SyncModeModel.allCases) { mode in
                        Text(mode.rawValue.capitalized).tag(mode)
                    }
                }
                Toggle("Show receive notifications", isOn: $workingCopy.showReceiveNotification)
            }

            Section("History") {
                Stepper("History size: \(workingCopy.historyLimit)", value: $workingCopy.historyLimit, in: 20...100)
                Stepper(
                    "Retained text bytes: \(workingCopy.maxHistoryTextBytes)",
                    value: $workingCopy.maxHistoryTextBytes,
                    in: 1024...262144,
                    step: 1024
                )
            }

            Section("Filtering") {
                Toggle("Sync text", isOn: $workingCopy.syncText)
                Toggle("Sync images", isOn: $workingCopy.syncImages)
                Toggle("Sync files", isOn: $workingCopy.syncFiles)
                Toggle("Block likely secrets", isOn: $workingCopy.blockSensitiveText)
                Stepper(
                    "Max payload: \(Int(workingCopy.maxPayloadBytes / 1024 / 1024)) MB",
                    value: Binding(
                        get: { Int(workingCopy.maxPayloadBytes / 1024 / 1024) },
                        set: { workingCopy.maxPayloadBytes = UInt64($0) * 1024 * 1024 }
                    ),
                    in: 1...128
                )
                VStack(alignment: .leading, spacing: 8) {
                    Text("Ignore patterns")
                    HStack {
                        TextField("Add substring to suppress syncing", text: $ignorePatternInput)
                        Button("Add") {
                            let trimmed = ignorePatternInput.trimmingCharacters(in: .whitespacesAndNewlines)
                            guard !trimmed.isEmpty else { return }
                            workingCopy.ignorePatterns.append(trimmed)
                            ignorePatternInput = ""
                        }
                    }
                    ForEach(workingCopy.ignorePatterns, id: \.self) { pattern in
                        HStack {
                            Text(pattern)
                            Spacer()
                            Button(role: .destructive) {
                                workingCopy.ignorePatterns.removeAll { $0 == pattern }
                            } label: {
                                Image(systemName: "trash")
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }

            Section("Network") {
                TextField("Port", value: $workingCopy.port, format: .number)
                Stepper(
                    "Clipboard poll: \(workingCopy.clipboardPollMs) ms",
                    value: Binding(
                        get: { Int(workingCopy.clipboardPollMs) },
                        set: { workingCopy.clipboardPollMs = UInt64($0) }
                    ),
                    in: 50...1000,
                    step: 25
                )
                Stepper(
                    "Duplicate window: \(workingCopy.smartSyncDuplicateWindowMs) ms",
                    value: Binding(
                        get: { Int(workingCopy.smartSyncDuplicateWindowMs) },
                        set: { workingCopy.smartSyncDuplicateWindowMs = UInt64($0) }
                    ),
                    in: 250...5000,
                    step: 50
                )
                Stepper(
                    "Debounce window: \(workingCopy.smartSyncDebounceMs) ms",
                    value: Binding(
                        get: { Int(workingCopy.smartSyncDebounceMs) },
                        set: { workingCopy.smartSyncDebounceMs = UInt64($0) }
                    ),
                    in: 50...1000,
                    step: 25
                )
            }

            Section {
                HStack {
                    Spacer()
                    Button("Reload") { if let settings = store.settings { workingCopy = settings } }
                    Button("Save Changes") { store.saveSettings(workingCopy) }
                        .buttonStyle(.borderedProminent)
                }
            }
        }
        .formStyle(.grouped)
        .padding(20)
        .frame(minWidth: 520, minHeight: 640)
        .onAppear {
            if let settings = store.settings {
                workingCopy = settings
            }
        }
    }
}
