import SwiftUI
import UniformTypeIdentifiers

struct DashboardRootView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var renameTarget: ManagedDevice?
    @State private var renameDraft = ""

    var body: some View {
        ZStack(alignment: .topTrailing) {
            HStack(spacing: 0) {
                sidebar
                Divider()
                content
            }
            .background(
                LinearGradient(
                    colors: [PBTheme.backgroundTop, PBTheme.backgroundBottom],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
            )

            ToastStackView(toasts: store.toasts)
                .padding(18)
        }
        .sheet(item: $renameTarget) { device in
            RenameDeviceSheet(
                device: device,
                draft: renameDraft,
                onCancel: { renameTarget = nil },
                onSave: { updatedName in
                    store.rename(device, to: updatedName)
                    renameTarget = nil
                }
            )
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 8) {
                Text("ClipRelay")
                    .font(.system(size: 30, weight: .bold, design: .serif))
                    .foregroundStyle(.white)
                Text(store.connectionBanner)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.white.opacity(0.72))
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
                if let status = store.status {
                    Text("\(status.peerCount) device\(status.peerCount == 1 ? "" : "s") nearby")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(.white.opacity(0.56))
                        .lineLimit(1)
                }
            }

            VStack(spacing: 10) {
                ForEach(DashboardSection.allCases) { section in
                    Button {
                        store.selectedSection = section
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: icon(for: section))
                                .frame(width: 18)
                            Text(section.title)
                            Spacer()
                        }
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 12)
                        .background(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .fill(
                                    store.selectedSection == section
                                        ? Color.white.opacity(0.12)
                                        : Color.clear
                                )
                        )
                    }
                    .buttonStyle(.plain)
                }
            }

            Spacer()

            VStack(alignment: .leading, spacing: 10) {
                PBBadge(store.settings?.syncEnabled == false ? "SYNC PAUSED" : "LOCAL-FIRST", tint: PBTheme.accentGreen, dark: true)
                Text("Clipboard history stays on your devices and moves over your local network.")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.white.opacity(0.66))
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(22)
        .frame(width: 220)
        .background(
            LinearGradient(
                colors: [PBTheme.sidebarTop, PBTheme.sidebarBottom],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
    }

    @ViewBuilder
    private var content: some View {
        switch store.selectedSection {
        case .timeline:
            TimelineSectionView(store: store)
        case .devices:
            DevicesSectionView(
                store: store,
                devices: store.devices,
                rename: beginRename(device:)
            )
        case .trust:
            TrustSectionView(
                store: store,
                rename: beginRename(device:)
            )
        case .settings:
            PreferencesView(store: store)
        }
    }

    private func beginRename(device: ManagedDevice) {
        renameDraft = device.name
        renameTarget = device
    }

    private func icon(for section: DashboardSection) -> String {
        switch section {
        case .timeline: return "clock.arrow.circlepath"
        case .devices: return "desktopcomputer"
        case .trust: return "checkmark.shield"
        case .settings: return "slider.horizontal.3"
        }
    }
}

private struct TimelineSectionView: View {
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        PBPanel {
            VStack(alignment: .leading, spacing: 18) {
                DashboardHeaderView(
                    eyebrow: "Timeline",
                    title: "Recent clipboard activity",
                    subtitle: "Copy items locally, resend them to another device, or keep important entries pinned."
                )

                if store.timeline.isEmpty {
                    EmptySectionView(
                        systemImage: "doc.text.magnifyingglass",
                        title: "Nothing here yet",
                        subtitle: "Copied text, images, and files will show up once the daemon starts receiving activity."
                    )
                } else {
                    ScrollView {
                        LazyVStack(spacing: 12) {
                            ForEach(store.timeline.prefix(80)) { item in
                                TimelineCardView(item: item, store: store)
                            }
                        }
                        .padding(.vertical, 2)
                    }
                }
            }
        }
        .padding(22)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct DevicesSectionView: View {
    @ObservedObject var store: ClipRelayStore
    let devices: [ManagedDevice]
    let rename: (ManagedDevice) -> Void
    @State private var showingFileImporter = false
    @State private var pendingFileTarget: ManagedDevice?

    var body: some View {
        PBPanel {
            VStack(alignment: .leading, spacing: 18) {
                DashboardHeaderView(
                    eyebrow: "Devices",
                    title: "Manage nearby devices",
                    subtitle: "Connect manually, rename trusted devices, and control active sessions."
                )

                ManualConnectCard(store: store)
                FileShareCard(store: store) { target in
                    pendingFileTarget = target
                    showingFileImporter = true
                }

                if devices.isEmpty {
                    EmptySectionView(
                        systemImage: "wifi.slash",
                        title: "No devices discovered",
                        subtitle: "When another ClipRelay device appears on your network, it will show up here."
                    )
                } else {
                    ScrollView {
                        LazyVStack(spacing: 12) {
                            ForEach(devices) { device in
                                DeviceManagementCard(device: device, store: store, rename: rename)
                            }
                        }
                        .padding(.vertical, 2)
                    }
                }
            }
        }
        .padding(22)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .fileImporter(
            isPresented: $showingFileImporter,
            allowedContentTypes: [.item],
            allowsMultipleSelection: false
        ) { result in
            guard case let .success(urls) = result, let url = urls.first else { return }
            store.sendFile(url: url, to: pendingFileTarget)
            pendingFileTarget = nil
        }
    }
}

private struct TrustSectionView: View {
    @ObservedObject var store: ClipRelayStore
    let rename: (ManagedDevice) -> Void

    private var attentionDevices: [ManagedDevice] {
        store.devices.filter { $0.trustState != .trusted }
    }

    var body: some View {
        PBPanel {
            VStack(alignment: .leading, spacing: 18) {
                DashboardHeaderView(
                    eyebrow: "Trust",
                    title: "Review device trust",
                    subtitle: "Approve devices you control, reject unknown ones, and revisit trusted peers."
                )

                if store.devices.isEmpty {
                    EmptySectionView(
                        systemImage: "checkmark.shield",
                        title: "No trust prompts right now",
                        subtitle: "New devices will appear here when they request access."
                    )
                } else {
                    ScrollView {
                        LazyVStack(spacing: 12) {
                            if !attentionDevices.isEmpty {
                                ForEach(attentionDevices) { device in
                                    DeviceManagementCard(device: device, store: store, rename: rename, emphasizeTrust: true)
                                }
                            }
                            if attentionDevices.isEmpty {
                                EmptySectionView(
                                    systemImage: "checkmark.shield",
                                    title: "All visible devices are trusted",
                                    subtitle: "You can still rename, revoke, or disconnect any device from the Devices tab."
                                )
                            }
                        }
                        .padding(.vertical, 2)
                    }
                }
            }
        }
        .padding(22)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct TimelineCardView: View {
    let item: TimelineItem
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: item.iconName)
                    .font(.system(size: 16, weight: .bold))
                    .foregroundStyle(PBTheme.accentBlue)
                    .frame(width: 30, height: 30)
                    .background(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(PBTheme.accentBlue.opacity(0.10))
                    )
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(PBTheme.ink)
                        .lineLimit(2)
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 6) {
                            Text(item.typeLabel)
                            Text("•")
                            Text(item.sourceDevice)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Text("•")
                            Text(item.timestamp.relativeTimeString())
                        }
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(item.typeLabel) from \(item.sourceDevice)")
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Text(item.timestamp.relativeTimeString())
                        }
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(PBTheme.inkSoft)
                }

                Spacer()
                if item.pinned {
                    PBBadge("PINNED", tint: PBTheme.accentGold)
                }
            }

            ViewThatFits(in: .horizontal) {
                timelineActions
                VStack(alignment: .leading, spacing: 8) {
                    timelineActions
                }
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(PBTheme.surfaceStrong)
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(PBTheme.stroke, lineWidth: 1)
                )
        )
    }

    @ViewBuilder
    private var timelineActions: some View {
        HStack(spacing: 8) {
            if item.fullText != nil {
                Button("Copy to this Mac") { store.copyTimelineItem(item) }
                    .buttonStyle(PBPrimaryButtonStyle())
            }

            Menu("Send") {
                Button("Send to all devices") { store.sendTimelineItem(item, to: nil) }
                ForEach(store.connectedDevices) { device in
                    Button(device.name) { store.sendTimelineItem(item, to: device) }
                }
            }
            .menuStyle(.borderlessButton)

            Button(item.pinned ? "Unpin" : "Pin") {
                store.pinTimelineItem(item, pinned: !item.pinned)
            }
            .buttonStyle(PBSecondaryButtonStyle())

            Button("Delete") {
                store.deleteTimelineItem(item)
            }
            .buttonStyle(PBSecondaryButtonStyle())
        }
    }
}

private struct DeviceManagementCard: View {
    let device: ManagedDevice
    @ObservedObject var store: ClipRelayStore
    let rename: (ManagedDevice) -> Void
    var emphasizeTrust = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill((emphasizeTrust ? PBTheme.accentOrange : PBTheme.accentBlue).opacity(0.12))
                    .frame(width: 40, height: 40)
                    .overlay(
                        Image(systemName: "desktopcomputer")
                            .font(.system(size: 16, weight: .bold))
                            .foregroundStyle(emphasizeTrust ? PBTheme.accentOrange : PBTheme.accentBlue)
                    )

                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Text(device.name)
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundStyle(PBTheme.ink)
                            .lineLimit(1)
                            .truncationMode(.tail)
                        DevicePill(text: device.connectionState.label, tint: device.connectionState.color)
                        DevicePill(text: device.trustState.rawValue.capitalized, tint: device.trustState.color)
                    }

                    if device.rawName != device.name {
                        Text(device.rawName)
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(PBTheme.inkSoft)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }

                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 10) {
                            if let endpoint = device.endpoint {
                                Label(endpoint, systemImage: "network")
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                            }
                            if let lastSeen = device.lastSeen {
                                Label("Seen \(lastSeen.relativeTimeString())", systemImage: "clock")
                                    .lineLimit(1)
                            }
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            if let endpoint = device.endpoint {
                                Label(endpoint, systemImage: "network")
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                            }
                            if let lastSeen = device.lastSeen {
                                Label("Seen \(lastSeen.relativeTimeString())", systemImage: "clock")
                                    .lineLimit(1)
                            }
                        }
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(PBTheme.inkSoft)

                    if let fingerprint = device.fingerprint {
                        Text("Fingerprint: \(fingerprint)")
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundStyle(PBTheme.inkSoft)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    if let error = device.lastError, !error.isEmpty {
                        Text(error)
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(PBTheme.accentPurple)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }

            ViewThatFits(in: .horizontal) {
                deviceActions
                VStack(alignment: .leading, spacing: 8) {
                    deviceActions
                }
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(PBTheme.surfaceStrong)
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(PBTheme.stroke, lineWidth: 1)
                )
        )
    }

    @ViewBuilder
    private var deviceActions: some View {
        HStack(spacing: 8) {
            if device.isConnected {
                Button("Disconnect") { store.disconnect(device) }
                    .buttonStyle(PBPrimaryButtonStyle(tint: PBTheme.accentOrange))
            }
            if device.trustState != .trusted {
                Button("Trust") { store.trust(device) }
                    .buttonStyle(PBPrimaryButtonStyle(tint: PBTheme.accentGreen))
                Button("Reject") { store.reject(device) }
                    .buttonStyle(PBSecondaryButtonStyle())
            } else {
                Button("Rename") { rename(device) }
                    .buttonStyle(PBSecondaryButtonStyle())
                Button("Revoke Trust") { store.revoke(device) }
                    .buttonStyle(PBSecondaryButtonStyle())
            }
        }
    }
}

private struct ManualConnectCard: View {
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Manual connect")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(PBTheme.ink)

            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    TextField("192.168.1.20:47823", text: $store.manualConnectAddress)
                        .pbInput()
                    Button("Connect") {
                        store.connectManual()
                    }
                    .buttonStyle(PBPrimaryButtonStyle())
                }
                VStack(alignment: .leading, spacing: 10) {
                    TextField("192.168.1.20:47823", text: $store.manualConnectAddress)
                        .pbInput()
                    Button("Connect") {
                        store.connectManual()
                    }
                    .buttonStyle(PBPrimaryButtonStyle())
                }
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(PBTheme.surfaceStrong)
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(PBTheme.stroke, lineWidth: 1)
                )
        )
    }
}

private struct FileShareCard: View {
    @ObservedObject var store: ClipRelayStore
    let chooseTarget: (ManagedDevice?) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Send a file")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(PBTheme.ink)

            Text("Pick any document, image, or archive and push it directly to nearby ClipRelay devices.")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(PBTheme.inkSoft)
                .fixedSize(horizontal: false, vertical: true)

            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) {
                    Button("Send to all devices") {
                        chooseTarget(nil)
                    }
                    .buttonStyle(PBPrimaryButtonStyle())

                    if !store.connectedDevices.isEmpty {
                        Menu("Send to device") {
                            ForEach(store.connectedDevices) { device in
                                Button(device.name) { chooseTarget(device) }
                            }
                        }
                        .menuStyle(.borderlessButton)
                    }
                }

                VStack(alignment: .leading, spacing: 10) {
                    Button("Send to all devices") {
                        chooseTarget(nil)
                    }
                    .buttonStyle(PBPrimaryButtonStyle())

                    if !store.connectedDevices.isEmpty {
                        Menu("Send to device") {
                            ForEach(store.connectedDevices) { device in
                                Button(device.name) { chooseTarget(device) }
                            }
                        }
                        .menuStyle(.borderlessButton)
                    }
                }
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(PBTheme.surfaceStrong)
                .overlay(
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .stroke(PBTheme.stroke, lineWidth: 1)
                )
        )
    }
}

private struct DashboardHeaderView: View {
    let eyebrow: String
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(eyebrow.uppercased())
                .font(.system(size: 11, weight: .bold))
                .tracking(0.5)
                .foregroundStyle(PBTheme.accentBlue)
            Text(title)
                .font(.system(size: 28, weight: .bold, design: .serif))
                .foregroundStyle(PBTheme.ink)
            Text(subtitle)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(PBTheme.inkSoft)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct EmptySectionView: View {
    let systemImage: String
    let title: String
    let subtitle: String

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: systemImage)
                .font(.system(size: 30, weight: .medium))
                .foregroundStyle(PBTheme.accentBlue)
            Text(title)
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(PBTheme.ink)
            Text(subtitle)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(PBTheme.inkSoft)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.vertical, 40)
    }
}

private struct DevicePill: View {
    let text: String
    let tint: Color

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .bold))
            .foregroundStyle(tint)
            .lineLimit(1)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                Capsule(style: .continuous)
                    .fill(tint.opacity(0.12))
            )
    }
}

private struct ToastStackView: View {
    let toasts: [ToastItem]

    var body: some View {
        VStack(alignment: .trailing, spacing: 10) {
            ForEach(toasts.suffix(3)) { toast in
                HStack(spacing: 10) {
                    Circle()
                        .fill(toast.tint)
                        .frame(width: 10, height: 10)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(toast.title)
                            .font(.system(size: 13, weight: .semibold))
                        Text(toast.body)
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(12)
                .background(
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .fill(Color.white.opacity(0.96))
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(PBTheme.stroke, lineWidth: 1)
                        )
                )
            }
        }
    }
}

private struct RenameDeviceSheet: View {
    let device: ManagedDevice
    @State var draft: String
    let onCancel: () -> Void
    let onSave: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Rename Device")
                .font(.system(size: 20, weight: .bold))
            Text("Choose a friendly name for \(device.name).")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(.secondary)
            TextField("Device name", text: $draft)
                .pbInput()
            HStack {
                Spacer()
                Button("Cancel", action: onCancel)
                    .buttonStyle(PBSecondaryButtonStyle())
                Button("Save") { onSave(draft) }
                    .buttonStyle(PBPrimaryButtonStyle())
            }
        }
        .padding(22)
        .frame(width: 360)
    }
}
