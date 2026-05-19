import SwiftUI
import UniformTypeIdentifiers
import Foundation
import SystemConfiguration
import CoreImage.CIFilterBuiltins

// MARK: - Root

struct DashboardRootView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var renameTarget:   ManagedDevice?
    @State private var renameDraft     = ""
    @State private var density: CRDensityMode = .comfortable
    @State private var columnVisibility: NavigationSplitViewVisibility = .all

    private var pendingContinuityItems: [IpcActivityEntry] {
        store.activityFeed.filter(\.isApplicable)
    }

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            Sidebar(store: store)
                .navigationSplitViewColumnWidth(min: 210, ideal: 226, max: 250)
        } detail: {
            DetailContent(store: store, density: $density, beginRename: beginRename)
        }
        .overlay(alignment: .bottomTrailing) {
            if !pendingContinuityItems.isEmpty {
                ContinuityStagingDrawer(entries: Array(pendingContinuityItems.prefix(3)), store: store)
                    .padding(.trailing, 26)
                    .padding(.bottom, 24)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .sheet(item: $renameTarget) { device in
            RenameDeviceSheet(
                device: device, draft: renameDraft,
                onCancel: { renameTarget = nil },
                onSave:   { store.rename(device, to: $0); renameTarget = nil }
            )
        }
    }

    private func beginRename(_ device: ManagedDevice) {
        renameDraft = device.name; renameTarget = device
    }
}

// MARK: - Sidebar

private struct Sidebar: View {
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        ZStack {
            CRHUDMaterial().ignoresSafeArea()

            VStack(alignment: .leading, spacing: 0) {
                // Header Area
                HStack(spacing: 12) {
                    Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                        .font(.system(size: 22))
                        .foregroundStyle(CRTheme.brandElectric)
                    Text("ClipRelay")
                        .font(.system(size: 16, weight: .bold))
                    Spacer()
                }
                .padding(.horizontal, 20)
                .padding(.top, 24)
                .padding(.bottom, 28)

                // Navigation List
                VStack(spacing: 6) {
                    SidebarNavButton(icon: "square.grid.2x2", label: "Dashboard", badge: 0, isSelected: store.selectedSection == .dashboard) {
                        store.selectedSection = .dashboard
                    }
                    SidebarNavButton(icon: "clock.arrow.circlepath", label: "Clipboard History", badge: store.pendingClipboardCount, isSelected: store.selectedSection == .history) {
                        store.selectedSection = .history
                    }
                    SidebarNavButton(icon: "macbook.and.iphone", label: "Synced Devices", badge: store.connectedDevices.count, isSelected: store.selectedSection == .devices) {
                        store.selectedSection = .devices
                    }
                    SidebarNavButton(icon: "gearshape", label: "Settings", badge: 0, isSelected: store.selectedSection == .settings) {
                        store.selectedSection = .settings
                    }
                }
                .padding(.horizontal, 14)

                Spacer()
                
                // Bottom Sync Status Footer
                SidebarFooter(store: store)
            }
        }
    }
}


// MARK: - Sidebar Footer

private struct SidebarFooter: View {
    @ObservedObject var store: ClipRelayStore
    private var syncColor: Color { store.settings?.syncEnabled == false ? CRTheme.accentOrange : CRTheme.accentGreen }
    private var syncLabel: String { store.settings?.syncEnabled == false ? "PAUSED" : "SYNCING" }
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            CRDividerDark()
            HStack(spacing: 7) {
                SidebarStatPill(icon: "desktopcomputer",  value: "\(store.devices.count)", label: "peers")
                SidebarStatPill(icon: "doc.on.clipboard", value: "\(store.timeline.count)", label: "items")
            }
            HStack(spacing: 6) {
                Circle().fill(syncColor).frame(width: 5.5, height: 5.5)
                Text(syncLabel).font(.system(size: 10, weight: .bold)).tracking(0.6).foregroundStyle(syncColor)
                Spacer()
                Text("Local-first").font(.system(size: 10)).foregroundStyle(CRTheme.sidebarInkSubtle)
            }
        }
        .padding(.horizontal, 16).padding(.bottom, 20)
    }
}

// MARK: - Detail Content

private struct DetailContent: View {
    @ObservedObject var store: ClipRelayStore
    @Binding var density: CRDensityMode
    let beginRename: (ManagedDevice) -> Void

    var body: some View {
        VStack(spacing: 0) {
            DashboardCommandBar(store: store)
                .padding(.horizontal, 20)
                .padding(.top, 18)
                .padding(.bottom, 12)

            // Sticky section toolbar
            CRSectionToolbar(
                title:    store.selectedSection.title,
                subtitle: store.selectedSection.subtitle
            ) {
                toolbarActions
            }

            // Content — keyed so SwiftUI rebuilds on section change (enables transition)
            Group {
                switch store.selectedSection {
                case .dashboard: UnifiedDashboardView(store: store, density: density)
                case .history:   TimelineSectionView(store: store, density: density)
                case .devices:  DevicesSectionView(store: store, rename: beginRename)
                case .workflows: TrustSectionView(store: store, rename: beginRename)
                case .settings: PreferencesView(store: store)
                }
            }
            .id(store.selectedSection)
            .transition(.asymmetric(
                insertion: .opacity.combined(with: .move(edge: .trailing)),
                removal:   .opacity.combined(with: .move(edge: .leading))
            ))
            .animation(.crSpring, value: store.selectedSection)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(CRTheme.canvasGradient)
    }

    @ViewBuilder private var toolbarActions: some View {
        switch store.selectedSection {
        case .dashboard, .history:
            // Density toggle
            HStack(spacing: 4) {
                Button {
                    withAnimation(.crFast) { density = .comfortable }
                } label: {
                    Image(systemName: density == .comfortable ? "rectangle.grid.1x2.fill" : "rectangle.grid.1x2")
                        .font(.system(size: 13, weight: .medium))
                }
                .buttonStyle(.plain)
                .foregroundStyle(density == .comfortable ? CRTheme.brandElectric : CRTheme.inkSoft)

                Button {
                    withAnimation(.crFast) { density = .compact }
                } label: {
                    Image(systemName: density == .compact ? "list.bullet.fill" : "list.bullet")
                        .font(.system(size: 13, weight: .medium))
                }
                .buttonStyle(.plain)
                .foregroundStyle(density == .compact ? CRTheme.brandElectric : CRTheme.inkSoft)
            }

        case .devices:
            Button { store.scanForDevices() } label: {
                Label("Scan", systemImage: "antenna.radiowaves.left.and.right")
                    .font(.system(size: 12.5, weight: .medium))
            }
            .buttonStyle(CRSecondaryButtonStyle())

        case .workflows:
            if store.devices.filter({ $0.trustState == .untrusted }).count > 1 {
                Button("Reject All") { store.rejectAll() }
                    .buttonStyle(CRDestructiveButtonStyle())
            }

        case .settings:
            EmptyView()
        }
    }
}

// MARK: - Command Bar

private struct DashboardCommandBar: View {
    @ObservedObject var store: ClipRelayStore

    var body: some View {
        HStack(spacing: 12) {
            Button {
                store.openCommandPalette()
            } label: {
                HStack(spacing: 12) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(CRTheme.brandElectric.opacity(0.12))
                            .frame(width: 36, height: 36)
                        Image(systemName: "magnifyingglass")
                            .font(.system(size: 13.5, weight: .semibold))
                            .foregroundStyle(CRTheme.brandElectric)
                    }

                    VStack(alignment: .leading, spacing: 2) {
                        Text("Search commands, clipboard, and quick actions")
                            .font(.system(size: 13.5, weight: .semibold))
                            .foregroundStyle(CRTheme.ink)
                        Text("Open the continuity command bar to push text, reconnect nearby devices, or jump through history.")
                            .font(.system(size: 11.5))
                            .foregroundStyle(CRTheme.inkSoft)
                            .lineLimit(2)
                    }

                    Spacer(minLength: 0)

                    HStack(spacing: 6) {
                        KbdChip("⌘", dark: false)
                        KbdChip("K", dark: false)
                    }
                }
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .background {
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .fill(Color.white.opacity(0.62))
                    .overlay {
                        RoundedRectangle(cornerRadius: 18, style: .continuous)
                            .strokeBorder(CRTheme.stroke.opacity(0.42), lineWidth: 0.5)
                    }
            }

            if store.connectedCount > 0 {
                Button {
                    store.sendCurrentClipboard(to: nil)
                } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Push active text")
                            .font(.system(size: 12.5, weight: .semibold))
                        Text("All peers")
                            .font(.system(size: 10.5))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                    .foregroundStyle(CRTheme.ink)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 13)
                    .background {
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .fill(Color.white.opacity(0.72))
                            .overlay {
                                RoundedRectangle(cornerRadius: 16, style: .continuous)
                                    .strokeBorder(CRTheme.stroke.opacity(0.42), lineWidth: 0.5)
                            }
                    }
                }
                .buttonStyle(.plain)
            }
        }
    }
}

// MARK: - Companion Card

private struct CompanionDeviceCard: View {
    let device: ManagedDevice
    let connectedPeers: Int
    @State private var isPulsing = false

    var body: some View {
        HStack(spacing: 16) {
            ZStack {
                Circle()
                    .fill(CRTheme.brandElectric.opacity(0.10))
                    .frame(width: 72, height: 72)
                    .scaleEffect(isPulsing ? 1.06 : 0.94)
                    .opacity(isPulsing ? 0.38 : 0.18)
                Circle()
                    .strokeBorder(CRTheme.brandElectric.opacity(0.16), lineWidth: 1)
                    .frame(width: 72, height: 72)
                    .scaleEffect(isPulsing ? 1.12 : 0.98)

                HStack(spacing: 8) {
                    Image(systemName: "laptopcomputer")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(CRTheme.ink.opacity(0.72))
                    Image(systemName: "iphone.gen3")
                        .font(.system(size: 21, weight: .semibold))
                        .foregroundStyle(CRTheme.brandElectric)
                }
            }

            VStack(alignment: .leading, spacing: 5) {
                Text("Companion nearby")
                    .font(.system(size: 10.5, weight: .bold))
                    .tracking(1.1)
                    .foregroundStyle(CRTheme.brandElectric)
                Text(device.name)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(CRTheme.ink)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    StatusDot(isOnline: device.isConnected, size: 6)
                    Text(device.connectionState == .connected ? "Clipboard ready" : device.connectionState.label)
                        .font(.system(size: 11.5, weight: .medium))
                        .foregroundStyle(CRTheme.inkSoft)
                    if connectedPeers > 1 {
                        Text("·")
                            .foregroundStyle(CRTheme.inkFaint)
                        Text("+\(connectedPeers - 1) more")
                            .font(.system(size: 11.5))
                            .foregroundStyle(CRTheme.inkSubtle)
                    }
                }
            }
        }
        .padding(16)
        .frame(width: 312, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(CRTheme.cardGradient)
                .overlay {
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .strokeBorder(CRTheme.brandElectric.opacity(0.50), lineWidth: 1.0)
                }
                .modifier(GlowModifier(color: CRTheme.brandElectric, radius: 18))
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) {
                isPulsing = true
            }
        }
    }
}

// MARK: - Staging Drawer

private struct ContinuityStagingDrawer: View {
    let entries: [IpcActivityEntry]
    @ObservedObject var store: ClipRelayStore

    private var leadEntry: IpcActivityEntry? { entries.first }

    var body: some View {
        if let leadEntry {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 10) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(CRTheme.brandElectric.opacity(0.10))
                            .frame(width: 38, height: 38)
                        Image(systemName: leadEntry.text_preview.map(isLikelyURL) == true ? "link" : "text.cursor")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(CRTheme.brandElectric)
                    }

                    VStack(alignment: .leading, spacing: 3) {
                        Text(isLikelyOTP(leadEntry.text_preview) ? "One-time code ready" : "Calm continuity staging")
                            .font(.system(size: 13.5, weight: .semibold))
                            .foregroundStyle(CRTheme.ink)
                        Text(leadEntry.device_name)
                            .font(.system(size: 11.5, weight: .medium))
                            .foregroundStyle(CRTheme.inkSoft)
                    }

                    Spacer(minLength: 0)

                    if entries.count > 1 {
                        Text("+\(entries.count - 1)")
                            .font(.system(size: 11, weight: .bold, design: .rounded))
                            .foregroundStyle(CRTheme.brandElectric)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background {
                                Capsule().fill(CRTheme.brandElectric.opacity(0.10))
                            }
                    }
                }

                if let preview = leadEntry.text_preview, !preview.isEmpty {
                    Text(preview)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(CRTheme.inkSoft)
                        .lineLimit(3)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 9)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background {
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(Color.white.opacity(0.6))
                                .overlay {
                                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                                        .strokeBorder(CRTheme.stroke.opacity(0.36), lineWidth: 0.5)
                                }
                        }
                }

                HStack(spacing: 8) {
                    Button(isLikelyOTP(leadEntry.text_preview) ? "Copy code" : "Copy") {
                        NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .default)
                        Task { await store.applyClipboard(entry: leadEntry) }
                    }
                    .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))

                    if let preview = leadEntry.text_preview,
                       let url = URL(string: preview.trimmingCharacters(in: .whitespacesAndNewlines)),
                       isLikelyURL(preview)
                    {
                        Button("Open Link") {
                            NSWorkspace.shared.open(url)
                        }
                        .buttonStyle(CRSecondaryButtonStyle())
                    }

                    Spacer(minLength: 0)
                }
            }
            .padding(16)
            .frame(width: 336, alignment: .leading)
            .background {
                RoundedRectangle(cornerRadius: 22, style: .continuous)
                    .fill(Color(hex: 0xF7F1E8, opacity: 0.96))
                    .overlay {
                        RoundedRectangle(cornerRadius: 22, style: .continuous)
                            .strokeBorder(Color(hex: 0xDED3C7, opacity: 0.98), lineWidth: 1)
                    }
                    .shadow(color: .black.opacity(0.07), radius: 24, x: 0, y: 10)
            }
        }
    }

    private func isLikelyOTP(_ text: String?) -> Bool {
        guard let text else { return false }
        let condensed = text.lowercased()
        let digitCount = text.filter(\.isNumber).count
        return digitCount >= 6 && digitCount <= 8 && ["otp", "code", "auth", "verify"].contains { condensed.contains($0) }
    }

    private func isLikelyURL(_ text: String?) -> Bool {
        guard let text else { return false }
        let value = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return value.hasPrefix("http://") || value.hasPrefix("https://")
    }
}

// MARK: - Timeline Section

private struct TimelineSectionView: View {
    @ObservedObject var store: ClipRelayStore
    let density: CRDensityMode
    @State private var search     = ""
    @State private var filterKind = "all"
    private let filters = [("all","All"),("text","Text"),("image","Image"),("file","File")]

    private var pinnedItems: [TimelineItem] {
        guard search.isEmpty else { return [] }
        return store.timeline.filter { $0.pinned }
    }

    private var filteredItems: [TimelineItem] {
        var base = store.timeline.filter { !$0.pinned }
        if filterKind != "all" { base = base.filter { $0.typeLabel.lowercased().contains(filterKind) } }
        if !search.isEmpty {
            base = base.filter {
                $0.title.localizedCaseInsensitiveContains(search)         ||
                $0.sourceDevice.localizedCaseInsensitiveContains(search)  ||
                ($0.fullText?.localizedCaseInsensitiveContains(search) ?? false)
            }
        }
        return base
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search + filter bar (below sticky toolbar, scrolls with content on small windows)
            VStack(alignment: .leading, spacing: 9) {
                CRSearchField(placeholder: "Search timeline…", text: $search)
                filterRow
            }
            .padding(.horizontal, 20).padding(.vertical, 12)
            .background(CRTheme.surfaceElevated.opacity(0.7))

            CRDivider()

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0, pinnedViews: []) {
                    VStack(alignment: .leading, spacing: 0) {
                        // Pinned group
                        if !pinnedItems.isEmpty {
                            groupLabel("PINNED", icon: "pin.fill", tint: CRTheme.accentGold)
                                .padding(.horizontal, 20).padding(.top, 16).padding(.bottom, 6)
                            VStack(spacing: density.cardSpacing) {
                                ForEach(pinnedItems) { TimelineCard(item: $0, store: store, density: density) }
                            }
                            .padding(.horizontal, 20)
                            .padding(.bottom, 14)

                            groupLabel("RECENT", icon: "clock", tint: CRTheme.inkSubtle)
                                .padding(.horizontal, 20).padding(.bottom, 6)
                        }

                        // Main list
                        if filteredItems.isEmpty && store.timeline.isEmpty {
                            CREmptyState(
                                systemImage: "doc.text.magnifyingglass",
                                title: "Nothing here yet",
                                message: "Copied text, images, and files will appear once the daemon is running."
                            )
                        } else if filteredItems.isEmpty {
                            CREmptyState(
                                systemImage: "magnifyingglass",
                                title: "No results",
                                message: search.isEmpty ? "Try a different filter." : "No items match \"\(search)\".",
                                accent: CRTheme.accentIndigo,
                                actionLabel: "Clear search",
                                onAction: { search = "" }
                            )
                        } else {
                            VStack(spacing: density.cardSpacing) {
                                ForEach(filteredItems) { TimelineCard(item: $0, store: store, density: density) }
                            }
                            .padding(.horizontal, 20)
                            .padding(.top, pinnedItems.isEmpty ? 16 : 0)
                        }
                    }
                    .padding(.bottom, 24)
                }
            }
        }
    }

    private var filterRow: some View {
        HStack(spacing: 5) {
            ForEach(filters, id: \.0) { key, label in
                Button(label) { withAnimation(.crFast) { filterKind = key } }
                    .font(.system(size: 12, weight: filterKind == key ? .semibold : .regular))
                    .foregroundStyle(filterKind == key ? CRTheme.brandElectric : CRTheme.inkSoft)
                    .padding(.horizontal, 9).padding(.vertical, 4)
                    .background {
                        Capsule()
                            .fill(filterKind == key ? CRTheme.brandElectric.opacity(0.09) : CRTheme.surface)
                            .overlay {
                                Capsule().strokeBorder(
                                    filterKind == key ? CRTheme.brandElectric.opacity(0.20) : CRTheme.stroke.opacity(0.55),
                                    lineWidth: 0.5)
                            }
                    }
                    .buttonStyle(.plain).animation(.crFast, value: filterKind)
            }
            Spacer()
            if !search.isEmpty || filterKind != "all" {
                Text("\(filteredItems.count) result\(filteredItems.count == 1 ? "" : "s")")
                    .font(.system(size: 11)).foregroundStyle(CRTheme.inkSubtle)
                    .transition(.opacity)
            }
        }
        .animation(.crFast, value: search.isEmpty && filterKind == "all")
    }

    @ViewBuilder private func groupLabel(_ text: String, icon: String, tint: Color) -> some View {
        HStack(spacing: 5) {
            Image(systemName: icon).font(.system(size: 9, weight: .semibold)).foregroundStyle(tint)
            Text(text).font(.system(size: 10, weight: .bold)).tracking(1.0).foregroundStyle(tint)
        }
    }
}

// MARK: - Devices Section

private struct DevicesSectionView: View {
    @ObservedObject var store: ClipRelayStore
    let rename: (ManagedDevice) -> Void
    @State private var showingFileImporter = false
    @State private var pendingFileTarget:  ManagedDevice?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                // Quick-action row — full width stacked vertically so content isn't cramped
                VStack(spacing: 8) {
                    MagicLinkPairingCard(store: store)
                    ManualConnectCard(store: store)
                    FileShareCard(store: store) { pendingFileTarget = $0; showingFileImporter = true }
                }
                .padding(.top, 18)

                if store.devices.isEmpty {
                    CREmptyState(
                        systemImage: "wifi.slash", title: "No devices discovered",
                        message: "Nearby devices on Wi-Fi or hotspot will appear here. Remembered devices can be pulled back with a fresh scan.",
                        accent: CRTheme.brandElectric,
                        actionLabel: "Scan again",
                        onAction: { store.scanForDevices() }
                    )
                } else {
                    VStack(spacing: 7) {
                        ForEach(store.devices) { DeviceCard(device: $0, store: store, rename: rename) }
                    }
                }
            }
            .padding(.horizontal, 20).padding(.bottom, 24)
        }
        .fileImporter(isPresented: $showingFileImporter, allowedContentTypes: [.item]) { result in
            if case let .success(url) = result {
                store.sendFile(url: url, to: pendingFileTarget); pendingFileTarget = nil
            }
        }
    }
}

// MARK: - Trust Section

private struct TrustSectionView: View {
    @ObservedObject var store: ClipRelayStore
    let rename: (ManagedDevice) -> Void
    private var attention: [ManagedDevice] { store.devices.filter { $0.trustState != .trusted } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                // Attention banner
                if !attention.isEmpty {
                    HStack(spacing: 10) {
                        ZStack {
                            Circle().fill(CRTheme.accentOrange.opacity(0.10)).frame(width: 28, height: 28)
                            Image(systemName: "exclamationmark.triangle.fill")
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(CRTheme.accentOrange)
                        }
                        Text("\(attention.count) device\(attention.count == 1 ? "" : "s") awaiting your decision")
                            .font(.system(size: 13, weight: .semibold)).foregroundStyle(CRTheme.ink)
                        Spacer()
                    }
                    .padding(.horizontal, 14).padding(.vertical, 10)
                    .background {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(CRTheme.accentOrange.opacity(0.06))
                            .overlay {
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .strokeBorder(CRTheme.accentOrange.opacity(0.18), lineWidth: 0.5)
                            }
                    }
                    .padding(.top, 18)
                }

                if attention.isEmpty {
                    CREmptyState(
                        systemImage: "checkmark.shield.fill", title: "All clear",
                        message: "No trust prompts right now. New devices appear here when they request access.",
                        accent: CRTheme.accentGreen
                    )
                } else {
                    VStack(spacing: 7) {
                        ForEach(attention) { DeviceCard(device: $0, store: store, rename: rename, emphasizeTrust: true) }
                    }
                }
            }
            .padding(.horizontal, 20).padding(.bottom, 24)
        }
    }
}

// MARK: - Timeline Card

struct TimelineCard: View {
    let item:    TimelineItem
    @ObservedObject var store: ClipRelayStore
    var density: CRDensityMode = .comfortable
    @State private var isHovered = false

    private var accent: Color {
        switch item.iconName {
        case "doc.on.clipboard": return CRTheme.accentBlue
        case "photo":            return CRTheme.accentPurple
        case "doc.fill":         return CRTheme.accentIndigo
        case "wifi":             return CRTheme.accentGreen
        case "wifi.slash":       return CRTheme.inkSoft
        default:                 return CRTheme.accentBlue
        }
    }

    private var charCount: String? {
        guard let t = item.fullText, !t.isEmpty else { return nil }
        return t.count > 999 ? "\(t.count / 1000)k chars" : "\(t.count) chars"
    }

    var body: some View {
        HStack(spacing: 0) {
            // Left accent stripe
            RoundedRectangle(cornerRadius: 1.5).fill(accent)
                .frame(width: 2.5)
                .padding(.vertical, density.rowPadding)
                .padding(.leading, 11)

            VStack(alignment: .leading, spacing: density == .compact ? 5 : 8) {
                // Header
                HStack(alignment: .center, spacing: 9) {
                    if density == .comfortable {
                        CRIconChip(systemName: item.iconName, tint: accent, size: 28)
                    }

                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 6) {
                            Text(item.title)
                                .font(.system(size: density == .compact ? 12.5 : 13, weight: .semibold))
                                .foregroundStyle(CRTheme.ink).lineLimit(1)
                            if item.pinned {
                                Image(systemName: "pin.fill")
                                    .font(.system(size: 9)).foregroundStyle(CRTheme.accentGold.opacity(0.85))
                                    .rotationEffect(.degrees(45))
                            }
                        }
                        HStack(spacing: 4) {
                            CRTag(text: item.typeLabel, tint: accent)
                            if let cc = charCount {
                                Text("·").foregroundStyle(CRTheme.inkFaint).font(.system(size: 9))
                                Text(cc).foregroundStyle(CRTheme.inkSubtle)
                            }
                            Text("·").foregroundStyle(CRTheme.inkFaint).font(.system(size: 9))
                            Image(systemName: "desktopcomputer").font(.system(size: 9))
                                .foregroundStyle(CRTheme.inkSubtle)
                            Text(item.sourceDevice).lineLimit(1).truncationMode(.middle)
                            Text("·").foregroundStyle(CRTheme.inkFaint).font(.system(size: 9))
                            Text(item.timestamp.relativeTimeString())
                        }
                        .font(.system(size: 10.5)).foregroundStyle(CRTheme.inkSoft)
                    }

                    Spacer(minLength: 0)
                }

                // Text preview
                if let preview = item.fullText, !preview.isEmpty, density == .comfortable {
                    Text(preview)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(CRTheme.inkSoft)
                        .lineLimit(isHovered ? 4 : 1)
                        .padding(.horizontal, 9).padding(.vertical, 6)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background {
                            RoundedRectangle(cornerRadius: 6, style: .continuous)
                                .fill(CRTheme.surface)
                                .overlay {
                                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                                        .strokeBorder(CRTheme.stroke.opacity(0.40), lineWidth: 0.5)
                                }
                        }
                        .animation(.crSpring, value: isHovered)
                }

                // Action bar
                if isHovered {
                    HStack(spacing: 6) {
                        if item.fullText != nil {
                            Button("Copy") {
                                NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .default)
                                store.copyTimelineItem(item)
                            }
                                .buttonStyle(CRPrimaryButtonStyle())
                        }
                        Menu {
                            Button("Send to all devices") { store.sendTimelineItem(item, to: nil) }
                            if !store.connectedDevices.isEmpty { Divider() }
                            ForEach(store.connectedDevices) { d in
                                Button(d.name) { store.sendTimelineItem(item, to: d) }
                            }
                        } label: {
                            Label("Send", systemImage: "paperplane.fill")
                                .font(.system(size: 12, weight: .medium))
                        }
                        .buttonStyle(CRSecondaryButtonStyle()).menuIndicator(.hidden)

                        Button(item.pinned ? "Unpin" : "Pin") {
                            store.pinTimelineItem(item, pinned: !item.pinned)
                        }
                        .buttonStyle(CRSecondaryButtonStyle())

                        Spacer()

                        Button { store.deleteTimelineItem(item) } label: {
                            Image(systemName: "trash").font(.system(size: 11.5, weight: .medium))
                        }
                        .buttonStyle(CRDestructiveButtonStyle())
                    }
                    .transition(.opacity.combined(with: .move(edge: .bottom)))
                }
            }
            .padding(.horizontal, 11).padding(.vertical, density.rowPadding)
        }
        .background {
            RoundedRectangle(cornerRadius: density.cardRadius, style: .continuous)
                .fill(CRTheme.surfaceStrong)
                .overlay {
                    RoundedRectangle(cornerRadius: density.cardRadius, style: .continuous)
                        .strokeBorder(isHovered ? accent.opacity(0.28) : CRTheme.stroke.opacity(0.38), lineWidth: 0.5)
                }
                .shadow(color: .black.opacity(isHovered ? 0.08 : 0.04),
                        radius: isHovered ? 12 : 4, x: 0, y: isHovered ? 3 : 1)
        }
        .onHover { isHovered = $0 }
        .scaleEffect(isHovered ? 1.015 : 1.0)
        .animation(.crSpring, value: isHovered)
    }
}

// MARK: - Device Card

private struct DeviceCard: View {
    let device: ManagedDevice
    @ObservedObject var store: ClipRelayStore
    let rename: (ManagedDevice) -> Void
    var emphasizeTrust: Bool = false
    @State private var isHovered = false

    private var accent: Color { emphasizeTrust ? CRTheme.accentOrange : device.connectionState.color }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .center, spacing: 11) {
                DeviceAvatar(name: device.name, platform: nil, size: 38, color: accent)

                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 6) {
                        Text(device.name)
                            .font(.system(size: 13.5, weight: .semibold)).foregroundStyle(CRTheme.ink)
                            .lineLimit(1)
                        HStack(spacing: 3) {
                            StatusDot(isOnline: device.isConnected, size: 6)
                            Text(device.connectionState.label)
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(device.connectionState.color)
                        }
                        if let battery = store.peerBatteries.first(where: { $0.deviceId == device.id }) {
                            BatteryIndicatorPill(level: battery.level, charging: battery.charging)
                        }
                    }
                    HStack(spacing: 6) {
                        CRTag(text: device.trustState.rawValue.capitalized, tint: device.trustState.color)
                        if device.canReconnect {
                            CRTag(text: "Auto reconnect", tint: CRTheme.brandElectric)
                        }
                        if let ep = device.endpoint {
                            Text("·").foregroundStyle(CRTheme.inkFaint).font(.system(size: 10))
                            Label(ep, systemImage: "network")
                                .lineLimit(1).truncationMode(.middle)
                                .font(.system(size: 10.5)).foregroundStyle(CRTheme.inkSoft)
                        }
                        if let seen = device.lastSeen {
                            Text("·").foregroundStyle(CRTheme.inkFaint).font(.system(size: 10))
                            Text("Seen \(seen.relativeTimeString())")
                                .font(.system(size: 10.5)).foregroundStyle(CRTheme.inkSoft)
                        }
                    }
                }
                Spacer()
            }
            .padding(.horizontal, 14).padding(.vertical, 12)

            // Fingerprint (hover)
            if let fp = device.fingerprint, isHovered {
                CRDivider().padding(.horizontal, 14)
                HStack(spacing: 5) {
                    Image(systemName: "key.fill").font(.system(size: 9)).foregroundStyle(CRTheme.inkSubtle)
                    Text(fp).font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(CRTheme.inkSubtle).lineLimit(1).truncationMode(.middle)
                }
                .padding(.horizontal, 14).padding(.vertical, 6)
                .transition(.opacity)
            }

            // Error
            if let err = device.lastError, !err.isEmpty {
                CRDivider().padding(.horizontal, 14)
                Label(err, systemImage: "exclamationmark.triangle.fill")
                    .font(.system(size: 11)).foregroundStyle(CRTheme.accentOrange)
                    .padding(.horizontal, 14).padding(.vertical, 6)
            }

            // Actions
            CRDivider()
            HStack(spacing: 7) {
                if device.isConnected {
                    Button("Disconnect") { store.disconnect(device) }
                        .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentOrange))
                } else if device.canReconnect {
                    Button("Reconnect") { store.scanForDevices() }
                        .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
                }
                if device.trustState != .trusted {
                    Button("Trust")  { store.trust(device) }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
                    Button("Reject") { store.reject(device) }.buttonStyle(CRDestructiveButtonStyle())
                } else {
                    Button("Rename")       { rename(device) }.buttonStyle(CRSecondaryButtonStyle())
                    Button("Revoke Trust") { store.revoke(device) }.buttonStyle(CRDestructiveButtonStyle())
                }
                Spacer()
            }
            .padding(.horizontal, 14).padding(.vertical, 9)
        }
        .background {
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .fill(CRTheme.surfaceStrong)
                .overlay {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .strokeBorder(isHovered ? accent.opacity(0.30) : CRTheme.stroke.opacity(0.38), lineWidth: 0.5)
                }
                .shadow(color: .black.opacity(isHovered ? 0.07 : 0.04),
                        radius: isHovered ? 12 : 4, x: 0, y: isHovered ? 3 : 1)
        }
        .onHover { isHovered = $0 }
        .scaleEffect(isHovered ? 1.015 : 1.0)
        .animation(.crSpring, value: isHovered)
    }
}

// MARK: - Manual Connect Card

private struct ManualConnectCard: View {
    @ObservedObject var store: ClipRelayStore
    @State private var hovered = false
    var body: some View {
        HStack(spacing: 12) {
            CRIconChip(systemName: "network", tint: CRTheme.accentBlue, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text("Manual Connect").font(.system(size: 13, weight: .semibold)).foregroundStyle(CRTheme.ink)
                Text("Connect to a specific IP address and port").font(.system(size: 11.5)).foregroundStyle(CRTheme.inkSoft)
            }
            Spacer()
            TextField("192.168.1.20:47823", text: $store.manualConnectAddress)
                .crInput().frame(width: 160)
            Button("Connect") { store.connectManual() }.buttonStyle(CRPrimaryButtonStyle())
        }
        .padding(14).frame(maxWidth: .infinity)
        .crCard(cornerRadius: 11, highlighted: hovered)
        .onHover { hovered = $0 }.animation(.crFast, value: hovered)
    }
}

// MARK: - File Share Card

private struct FileShareCard: View {
    @ObservedObject var store: ClipRelayStore
    let chooseTarget: (ManagedDevice?) -> Void
    @State private var hovered = false
    var body: some View {
        HStack(spacing: 12) {
            CRIconChip(systemName: "arrow.up.doc.fill", tint: CRTheme.brandElectric, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text("Send a File").font(.system(size: 13, weight: .semibold)).foregroundStyle(CRTheme.ink)
                Text("Push a document, image, or archive to peers").font(.system(size: 11.5)).foregroundStyle(CRTheme.inkSoft)
            }
            Spacer()
            Button("Send to all") { chooseTarget(nil) }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
            if !store.connectedDevices.isEmpty {
                Menu("Choose…") {
                    ForEach(store.connectedDevices) { d in Button(d.name) { chooseTarget(d) } }
                }
                .buttonStyle(CRSecondaryButtonStyle())
            }
        }
        .padding(14).frame(maxWidth: .infinity)
        .crCard(cornerRadius: 11, highlighted: hovered, accent: CRTheme.brandElectric)
        .onHover { hovered = $0 }.animation(.crFast, value: hovered)
    }
}

// MARK: - Rename Sheet (auto-focuses text field)

private struct RenameDeviceSheet: View {
    let device: ManagedDevice
    @State var draft: String
    let onCancel: () -> Void
    let onSave:   (String) -> Void
    @FocusState private var fieldFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 3) {
                Text("Rename Device").font(.system(size: 18, weight: .bold)).foregroundStyle(CRTheme.ink)
                Text("Give \(device.rawName) a friendly name.")
                    .font(.system(size: 12.5)).foregroundStyle(CRTheme.inkSoft)
            }
            TextField("Device name", text: $draft)
                .crInput()
                .focused($fieldFocused)
                .onSubmit { if !draft.isEmpty { onSave(draft) } }
            HStack {
                Spacer()
                Button("Cancel", action: onCancel).buttonStyle(CRSecondaryButtonStyle())
                Button("Save") { onSave(draft) }.buttonStyle(CRPrimaryButtonStyle())
                    .disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(24).frame(width: 330)
        .background(CRTheme.surfaceStrong.ignoresSafeArea())
        .onAppear { fieldFocused = true }
    }
}

// MARK: - Unified Dashboard (Mockup Style)

struct UnifiedDashboardView: View {
    @ObservedObject var store: ClipRelayStore
    let density: CRDensityMode
    @State private var showingFilePicker = false
    @State private var isFileDragTargeted  = false
    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                // ── Section 1: Active Device Hero ─────────────────────────
                if let device = store.connectedDevices.first {
                    connectedHeroSection(device: device)
                } else {
                    noDeviceHero
                }

                // ── Section 2: Quick Action Row ───────────────────────────
                if !store.connectedDevices.isEmpty {
                    quickActionRow
                }

                // ── Section 3: Live Stats Row ─────────────────────────────
                statsRow
                // ── Section 4: Sync Controls ──────────────────────────────
                if store.settings != nil {
                    syncControlsSection
                }

                // ── Section 5: Recent Activity ────────────────────────────
                if !store.timeline.isEmpty {
                    recentActivitySection
                }
            }
            .padding(.horizontal, 40)
            .padding(.vertical, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .fileImporter(isPresented: $showingFilePicker, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result {
                for url in urls { store.sendFile(url: url, to: nil) }
            }
        }
    }

    // ── Connected Hero ────────────────────────────────────────────────────────

    @ViewBuilder
    private func connectedHeroSection(device: ManagedDevice) -> some View {
        VStack(spacing: 0) {
            // Top bar
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Welcome back, \(NSUserName())")
                        .font(.system(size: 22, weight: .bold))
                    HStack(spacing: 6) {
                        Circle().fill(CRTheme.accentGreen).frame(width: 7, height: 7)
                            .shadow(color: CRTheme.accentGreen.opacity(0.6), radius: 4)
                        Text("\(store.connectedDevices.count) device\(store.connectedDevices.count == 1 ? "" : "s") online")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                }
                Spacer()
                Button { store.scanForDevices() } label: {
                    Label("Scan", systemImage: "antenna.radiowaves.left.and.right")
                        .font(.system(size: 12.5, weight: .medium))
                }
                .buttonStyle(CRSecondaryButtonStyle())
            }
            .padding(.horizontal, 28)
            .padding(.top, 24)
            .padding(.bottom, 20)

            CRDivider().padding(.horizontal, 28)

            // Device row
            HStack(spacing: 24) {
                // Device icon + name
                HStack(spacing: 16) {
                    ZStack {
                        Circle()
                            .fill(CRTheme.brandElectric.opacity(0.12))
                            .frame(width: 52, height: 52)
                        Image(systemName: "iphone.gen3")
                            .font(.system(size: 22, weight: .semibold))
                            .foregroundStyle(CRTheme.brandElectric)
                    }
                    VStack(alignment: .leading, spacing: 3) {
                        Text(device.name)
                            .font(.system(size: 17, weight: .bold))
                        Text(device.lastSync?.relativeTimeString() ?? "Synced just now")
                            .font(.system(size: 12))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                }
                Spacer()
                // Clipboard push
                Button {
                    store.sendCurrentClipboard(to: device)
                } label: {
                    Label("Push clipboard", systemImage: "doc.on.clipboard")
                        .font(.system(size: 12.5, weight: .semibold))
                }
                .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))

                // Disconnect
                Button { store.disconnect(device) } label: {
                    Image(systemName: "xmark.circle")
                        .font(.system(size: 16))
                        .foregroundStyle(CRTheme.inkSoft)
                }
                .buttonStyle(.plain)
                .help("Disconnect \(device.name)")
            }
            .padding(.horizontal, 28)
            .padding(.vertical, 20)

            // More connected devices
            if store.connectedDevices.count > 1 {
                CRDivider().padding(.horizontal, 28)
                ForEach(store.connectedDevices.dropFirst()) { d in
                    HStack(spacing: 16) {
                        Circle().fill(CRTheme.accentGreen).frame(width: 7, height: 7)
                        Text(d.name).font(.system(size: 14, weight: .semibold))
                        Spacer()
                        Button("Push") { store.sendCurrentClipboard(to: d) }
                            .buttonStyle(CRSecondaryButtonStyle())
                    }
                    .padding(.horizontal, 28)
                    .padding(.vertical, 12)
                }
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.12) : .white)
                .shadow(color: .black.opacity(0.05), radius: 16, y: 4)
        }
    }

    // ── No-device Hero ────────────────────────────────────────────────────────

    private var noDeviceHero: some View {
        VStack(spacing: 20) {
            Image(systemName: "iphone.gen3.slash")
                .font(.system(size: 44))
                .foregroundStyle(CRTheme.brandElectric.opacity(0.6))
            Text("No Devices Connected")
                .font(.system(size: 20, weight: .bold))
            Text("Open ClipRelay on your iPhone or Android to start syncing clipboard, files and calls.")
                .font(.system(size: 14))
                .foregroundStyle(CRTheme.inkSoft)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 340)
            Button("Scan for Devices") { store.scanForDevices() }
                .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
        }
        .frame(maxWidth: .infinity)
        .padding(40)
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.12) : .white)
                .shadow(color: .black.opacity(0.04), radius: 16, y: 4)
        }
    }

    // ── Quick Action Row ──────────────────────────────────────────────────────

    private var quickActionRow: some View {
        LazyVGrid(columns: [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)], spacing: 14) {
            // Send File
            dashActionCard(
                icon: "arrow.up.doc.fill",
                title: "Send File",
                subtitle: "Drag & drop or pick",
                tint: CRTheme.brandElectric,
                isDragTarget: isFileDragTargeted
            ) {
                showingFilePicker = true
            }
            .dropDestination(for: URL.self) { urls, _ in
                urls.forEach { store.sendFile(url: $0, to: nil) }
                return !urls.isEmpty
            } isTargeted: { isFileDragTargeted = $0 }

            // Push Clipboard
            dashActionCard(
                icon: "doc.on.clipboard.fill",
                title: "Push Clipboard",
                subtitle: "All connected devices",
                tint: CRTheme.accentIndigo
            ) {
                store.sendCurrentClipboard(to: nil)
            }

            // View History
            dashActionCard(
                icon: "clock.arrow.circlepath",
                title: "History",
                subtitle: "\(store.timeline.count) items",
                tint: CRTheme.accentGold
            ) {
                store.selectedSection = .history
            }

            // Manage Devices
            dashActionCard(
                icon: "iphone.gen3",
                title: "Devices",
                subtitle: "\(store.devices.count) paired",
                tint: CRTheme.accentGreen
            ) {
                store.selectedSection = .devices
            }
        }
    }

    @ViewBuilder
    private func dashActionCard(
        icon: String, title: String, subtitle: String,
        tint: Color, isDragTarget: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(tint.opacity(isDragTarget ? 0.25 : 0.12))
                        .frame(width: 42, height: 42)
                    Image(systemName: icon)
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(tint)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 14, weight: .bold))
                        .foregroundStyle(Color.primary)
                    Text(subtitle)
                        .font(.system(size: 11.5))
                        .foregroundStyle(CRTheme.inkSoft)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background {
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(isDragTarget
                          ? tint.opacity(0.10)
                          : (colorScheme == .dark ? Color(white: 0.14) : .white))
                    .overlay {
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .strokeBorder(isDragTarget ? tint.opacity(0.5) : CRTheme.stroke.opacity(0.4), lineWidth: isDragTarget ? 2 : 0.5)
                    }
                    .shadow(color: .black.opacity(0.04), radius: 8, y: 3)
            }
        }
        .buttonStyle(.plain)
        .animation(.crFast, value: isDragTarget)
    }

    // ── Stats Row ─────────────────────────────────────────────────────────────

    private var statsRow: some View {
        HStack(spacing: 14) {
            statCard(value: "\(store.connectedDevices.count)", label: "Connected", icon: "wifi", tint: CRTheme.accentGreen)
            statCard(value: "\(store.devices.count)", label: "Paired", icon: "checkmark.shield.fill", tint: CRTheme.brandElectric)
            statCard(value: "\(store.timeline.count)", label: "Synced", icon: "doc.on.clipboard.fill", tint: CRTheme.accentIndigo)
            statCard(value: store.pendingClipboardCount > 0 ? "\(store.pendingClipboardCount)" : "—",
                     label: "Pending", icon: "tray.fill",
                     tint: store.pendingClipboardCount > 0 ? CRTheme.accentOrange : CRTheme.inkSubtle)
        }
    }

    @ViewBuilder
    private func statCard(value: String, label: String, icon: String, tint: Color) -> some View {
        HStack(spacing: 14) {
            Image(systemName: icon)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(tint)
                .frame(width: 32, height: 32)
                .background(tint.opacity(0.10), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(value)
                    .font(.system(size: 18, weight: .bold, design: .rounded))
                    .foregroundStyle(Color.primary)
                Text(label)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            Spacer()
        }
        .padding(16)
        .frame(maxWidth: .infinity)
        .background {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.12) : .white)
                .shadow(color: .black.opacity(0.04), radius: 8, y: 3)
        }
    }

    // ── Sync Controls ─────────────────────────────────────────────────────────

    private var syncControlsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Continuity Settings")
                .font(.system(size: 15, weight: .bold))

            HStack(spacing: 14) {
                syncToggleCard(title: "Text", icon: "text.alignleft", keyPath: \.syncText, tint: CRTheme.accentIndigo)
                syncToggleCard(title: "Images", icon: "photo", keyPath: \.syncImages, tint: CRTheme.accentGold)
                syncToggleCard(title: "Files", icon: "doc", keyPath: \.syncFiles, tint: CRTheme.brandElectric)
                syncToggleCard(title: "Block Sensitive", icon: "eye.slash.fill", keyPath: \.blockSensitiveText, tint: CRTheme.accentRed)
            }
        }
    }

    @ViewBuilder
    private func syncToggleCard(title: String, icon: String, keyPath: WritableKeyPath<ClipRelaySettingsSnapshot, Bool>, tint: Color) -> some View {
        let isOn = binding(for: keyPath)
        Button {
            isOn.wrappedValue.toggle()
        } label: {
            HStack(spacing: 12) {
                ZStack {
                    Circle()
                        .fill(isOn.wrappedValue ? tint.opacity(0.15) : CRTheme.inkSubtle.opacity(0.1))
                        .frame(width: 32, height: 32)
                    Image(systemName: icon)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(isOn.wrappedValue ? tint : CRTheme.inkSubtle)
                }

                Text(title)
                    .font(.system(size: 13, weight: .bold))
                    .foregroundStyle(isOn.wrappedValue ? Color.primary : CRTheme.inkSoft)

                Spacer()

                Toggle("", isOn: isOn)
                    .toggleStyle(.switch)
                    .labelsHidden()
                    .controlSize(.mini)
                    .tint(tint)
            }
            .padding(14)
            .background {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(colorScheme == .dark ? Color(white: isOn.wrappedValue ? 0.15 : 0.12) : (isOn.wrappedValue ? .white : Color(white: 0.96)))
                    .shadow(color: .black.opacity(isOn.wrappedValue ? 0.04 : 0.0), radius: 8, y: 3)
            }
        }
        .buttonStyle(.plain)
    }

    private func binding(for keyPath: WritableKeyPath<ClipRelaySettingsSnapshot, Bool>) -> Binding<Bool> {
        Binding(
            get: { store.settings?[keyPath: keyPath] ?? false },
            set: { newValue in
                guard var s = store.settings else { return }
                s[keyPath: keyPath] = newValue
                store.saveSettings(s)
            }
        )
    }

    // ── Recent Activity ───────────────────────────────────────────────────────

    private var recentActivitySection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Recent Activity")
                    .font(.system(size: 15, weight: .bold))
                Spacer()
                Button("See all") { store.selectedSection = .history }
                    .buttonStyle(.plain)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(CRTheme.brandElectric)
            }

            VStack(spacing: 6) {
                ForEach(store.timeline.prefix(5)) { item in
                    HStack(spacing: 14) {
                        Image(systemName: activityIcon(for: item))
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(activityTint(for: item))
                            .frame(width: 30, height: 30)
                            .background(activityTint(for: item).opacity(0.10),
                                        in: RoundedRectangle(cornerRadius: 8, style: .continuous))

                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.title)
                                .font(.system(size: 13, weight: .semibold))
                                .lineLimit(1)
                            Text(item.sourceDevice)
                                .font(.system(size: 11))
                                .foregroundStyle(CRTheme.inkSoft)
                        }

                        Spacer()

                        Text(item.timestamp.relativeTimeString())
                            .font(.system(size: 11))
                            .foregroundStyle(CRTheme.inkSubtle)

                        if let text = item.fullText, !text.isEmpty {
                            Button {
                                store.applyClipboardLocally(text: text)
                            } label: {
                                Text("Copy")
                                    .font(.system(size: 11, weight: .semibold))
                            }
                            .buttonStyle(CRSecondaryButtonStyle())
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background {
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(colorScheme == .dark ? Color(white: 0.13) : .white)
                    }
                }
            }
        }
    }

    private func activityIcon(for item: TimelineItem) -> String {
        let t = item.typeLabel.lowercased()
        if t.contains("image") { return "photo" }
        if t.contains("file")  { return "doc.fill" }
        return "doc.on.clipboard"
    }

    private func activityTint(for item: TimelineItem) -> Color {
        let t = item.typeLabel.lowercased()
        if t.contains("image") { return CRTheme.accentIndigo }
        if t.contains("file")  { return CRTheme.accentGold }
        return CRTheme.brandElectric
    }
}

// MARK: - Hero Device Layouts

private struct EmptyHeroCard: View {
    @ObservedObject var store: ClipRelayStore
    @Environment(\.colorScheme) var colorScheme
    
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "antenna.radiowaves.left.and.right")
                .font(.system(size: 48))
                .foregroundStyle(CRTheme.brandElectric)
            Text("No Devices Connected")
                .font(.system(size: 20, weight: .bold))
            Text("Open the app on your phone or tablet to start syncing.")
                .font(.system(size: 14))
                .foregroundStyle(Color.secondary)
            Button("Scan for devices") { store.scanForDevices() }
                .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
                .padding(.top, 8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.1) : .white)
                .shadow(color: .black.opacity(0.04), radius: 10, y: 4)
        }
    }
}

private struct HeroDeviceCard: View {
    let device: ManagedDevice
    @ObservedObject var store: ClipRelayStore
    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        HStack(spacing: 0) {
            // Left Half: Massive Visual Mockup
            ZStack {
                Rectangle()
                    .fill(Color.primary.opacity(0.03))
                
                // Device Mockup
                ZStack {
                    RoundedRectangle(cornerRadius: 32, style: .continuous)
                        .fill(Color.black.opacity(0.85))
                        .frame(width: 160, height: 320)
                        .shadow(color: .black.opacity(0.25), radius: 30, x: 0, y: 15)
                    
                    // Screen area
                    RoundedRectangle(cornerRadius: 24, style: .continuous)
                        .fill(
                            LinearGradient(colors: [CRTheme.brandElectric, CRTheme.brandViolet], startPoint: .topLeading, endPoint: .bottomTrailing)
                        )
                        .frame(width: 146, height: 306)
                        .overlay {
                            VStack {
                                Spacer()
                                Image(systemName: "applelogo")
                                    .font(.system(size: 48))
                                    .foregroundStyle(.white.opacity(0.9))
                                Spacer()
                            }
                        }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            
            // Right Half: Details and Actions
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Spacer()
                    Button(action: { store.disconnect(device) }) {
                        Label("Disconnect", systemImage: "link.badge.minus")
                            .font(.system(size: 13, weight: .medium))
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(Color.secondary)
                    
                    Button(action: {}) {
                        Image(systemName: "ellipsis")
                            .font(.system(size: 13, weight: .bold))
                            .rotationEffect(.degrees(90))
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(Color.secondary)
                    .padding(.leading, 12)
                }
                .padding(.bottom, 40)
                
                Text(device.name)
                    .font(.system(size: 32, weight: .bold))
                    .foregroundStyle(Color.primary)
                    .padding(.bottom, 12)
                
                HStack(spacing: 12) {
                    HStack(spacing: 4) {
                        Circle().fill(CRTheme.accentGreen).frame(width: 6, height: 6)
                        Text("Connected")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(CRTheme.accentGreen)
                    }
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(Capsule().fill(CRTheme.accentGreen.opacity(0.1)))
                    
                    HStack(spacing: 4) {
                        Image(systemName: "battery.100").foregroundStyle(Color.primary)
                        Text("100%")
                            .font(.system(size: 12, weight: .medium))
                    }
                }
                .padding(.bottom, 60)
                
                // Action Grid
                HStack(spacing: 0) {
                    HeroActionButton(icon: "folder", label: "Files") {
                        // Trigger file dialog
                    }
                    Spacer()
                    HeroActionButton(icon: "macwindow", label: "Screencast") {
                        // Placeholder
                    }
                    Spacer()
                    HeroActionButton(icon: "arrow.triangle.2.circlepath", label: "Content sync") {
                        store.selectedSection = .history
                    }
                }
                .padding(.trailing, 20)
                
                Spacer()
            }
            .padding(40)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(colorScheme == .dark ? Color(white: 0.12) : .white)
        }
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.12) : .white)
                .shadow(color: .black.opacity(0.06), radius: 15, y: 5)
        }
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

private struct HeroActionButton: View {
    let icon: String
    let label: String
    let action: () -> Void
    @State private var isHovered = false
    
    var body: some View {
        Button(action: action) {
            VStack(spacing: 12) {
                ZStack {
                    Circle()
                        .fill(isHovered ? CRTheme.brandElectric.opacity(0.1) : Color.primary.opacity(0.03))
                        .frame(width: 48, height: 48)
                    Image(systemName: icon)
                        .font(.system(size: 18))
                        .foregroundStyle(isHovered ? CRTheme.brandElectric : Color.primary)
                }
                Text(label)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(Color.secondary)
            }
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

private struct UnifiedDeviceCard: View {
    let device: ManagedDevice
    @Environment(\.colorScheme) var colorScheme
    @State private var isHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top) {
                Image(systemName: "iphone") // or dynamic based on device type
                    .font(.system(size: 24))
                    .foregroundStyle(CRTheme.brandElectric)
                
                Spacer()
                
                Text(device.connectionState == .connected ? "Active" : "Linked")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(CRTheme.brandElectric)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(device.name)
                    .font(.system(size: 18, weight: .bold))
                Text(device.connectionState == .connected ? "Continuity Active" : "Proximity Syncing...")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(Color.secondary)
            }

            CRDividerDark()

            VStack(spacing: 6) {
                HStack {
                    Label("Battery", systemImage: "battery.100")
                        .font(.system(size: 11))
                        .foregroundStyle(Color.secondary)
                    Spacer()
                    Text("100%")
                        .font(.system(size: 11, weight: .semibold))
                }
                HStack {
                    Label("Last Sync", systemImage: "clock")
                        .font(.system(size: 11))
                        .foregroundStyle(Color.secondary)
                    Spacer()
                    Text(device.lastSync?.relativeTimeString() ?? "Just now")
                        .font(.system(size: 11, weight: .semibold))
                }
            }

            HStack(spacing: 8) {
                Button("Copy Last") {}
                    .buttonStyle(CRSecondaryButtonStyle())
                Button("Manage") {}
                    .buttonStyle(CRSecondaryButtonStyle())
            }
        }
        .padding(20)
        .frame(width: 240)
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(colorScheme == .dark ? Color(white: 0.1) : .white)
        }
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.05), lineWidth: 1)
        }
        // Gradient Glow
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(LinearGradient(colors: [CRTheme.brandElectric.opacity(0.3), CRTheme.brandPink.opacity(0.3)], startPoint: .topLeading, endPoint: .bottomTrailing))
                .blur(radius: 20)
                .opacity(isHovered ? 1.0 : 0.4)
                .offset(y: 8)
        }
        .scaleEffect(isHovered ? 1.02 : 1.0)
        .animation(.crSpring, value: isHovered)
        .onHover { isHovered = $0 }
    }
}

// MARK: - Battery Indicator Pill
struct BatteryIndicatorPill: View {
    let level: Int
    let charging: Bool

    private var iconName: String {
        if charging {
            return "battery.100.bolt"
        }
        if level <= 15 { return "battery.0" }
        if level <= 35 { return "battery.25" }
        if level <= 65 { return "battery.50" }
        if level <= 85 { return "battery.75" }
        return "battery.100"
    }

    private var tintColor: Color {
        if charging { return CRTheme.accentGreen }
        if level <= 20 { return Color.red }
        if level <= 50 { return CRTheme.accentOrange }
        return CRTheme.accentGreen
    }

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: iconName)
                .font(.system(size: 9.5, weight: .semibold))
            Text("\(level)%")
                .font(.system(size: 9.5, weight: .bold, design: .rounded))
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .foregroundStyle(tintColor)
        .background(
            Capsule()
                .fill(tintColor.opacity(0.12))
        )
    }
}

// MARK: - Magic Link Pairing Card
struct MagicLinkPairingCard: View {
    @ObservedObject var store: ClipRelayStore
    @State private var hovered = false
    @State private var showingQR = false
    
    var body: some View {
        HStack(spacing: 12) {
            CRIconChip(systemName: "qrcode", tint: CRTheme.accentGreen, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text("Magic Link pairing").font(.system(size: 13, weight: .semibold)).foregroundStyle(CRTheme.ink)
                Text("Pair a phone or tablet instantly using a QR code").font(.system(size: 11.5)).foregroundStyle(CRTheme.inkSoft)
            }
            Spacer()
            Button("Show QR Code") { showingQR = true }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
        }
        .padding(14).frame(maxWidth: .infinity)
        .crCard(cornerRadius: 11, highlighted: hovered, accent: CRTheme.accentGreen)
        .onHover { hovered = $0 }.animation(.crFast, value: hovered)
        .sheet(isPresented: $showingQR) {
            QRCodePairingSheet(store: store)
        }
    }
}

// MARK: - QRCode Pairing Sheet View
struct QRCodePairingSheet: View {
    @ObservedObject var store: ClipRelayStore
    @Environment(\.dismiss) var dismiss
    @State private var localIP: String = "127.0.0.1"

    var pairingURL: String {
        let name = store.settings?.deviceName ?? HostName()
        let port = store.settings?.port ?? 47823
        let fp = store.localFingerprint ?? ""
        return "cliprelay://pair?name=\(name.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? "")&ip=\(localIP)&port=\(port)&fingerprint=\(fp)"
    }

    var pinCode: String {
        let fp = store.localFingerprint ?? "123456"
        let digits = fp.filter { $0.isNumber }
        if digits.count >= 6 {
            return String(digits.prefix(6))
        }
        let sum = fp.utf8.reduce(0, { $0 + Int($1) })
        return String(format: "%06d", sum % 1000000)
    }

    var body: some View {
        VStack(spacing: 20) {
            HStack {
                Text("Magic Link Pair")
                    .font(.system(size: 16, weight: .bold))
                    .foregroundStyle(CRTheme.ink)
                Spacer()
                Button(action: { dismiss() }) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 18))
                        .foregroundStyle(CRTheme.inkSubtle)
                }
                .buttonStyle(PlainButtonStyle())
            }

            Text("Scan the QR code below from the ClipRelay Android app to pair instantly, or use the 6-digit confirmation PIN.")
                .font(.system(size: 12))
                .foregroundStyle(CRTheme.inkSoft)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 10)

            if let qrImage = generateQRCode(from: pairingURL) {
                Image(nsImage: qrImage)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 180, height: 180)
                    .padding(8)
                    .background(Color.white)
                    .cornerRadius(12)
                    .shadow(color: Color.black.opacity(0.1), radius: 8, x: 0, y: 4)
            } else {
                ProgressView()
                    .frame(width: 180, height: 180)
            }

            VStack(spacing: 4) {
                Text("PAIRING PIN")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(CRTheme.inkSubtle)
                Text(pinCode)
                    .font(.system(size: 28, weight: .black, design: .monospaced))
                    .foregroundStyle(CRTheme.brandElectric)
                    .tracking(4)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 10)
            .background(CRTheme.surfaceElevated.opacity(0.6))
            .cornerRadius(12)
            .overlay {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(CRTheme.stroke.opacity(0.42), lineWidth: 0.5)
            }

            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("Device:")
                        .font(.system(size: 11, weight: .medium)).foregroundStyle(CRTheme.inkSoft)
                    Spacer()
                    Text(store.settings?.deviceName ?? HostName())
                        .font(.system(size: 11, weight: .semibold)).foregroundStyle(CRTheme.ink)
                }
                HStack {
                    Text("Network Address:")
                        .font(.system(size: 11, weight: .medium)).foregroundStyle(CRTheme.inkSoft)
                    Spacer()
                    Text("\(localIP):\(store.settings?.port ?? 47823)")
                        .font(.system(size: 11, weight: .semibold)).foregroundStyle(CRTheme.ink)
                }
            }
            .padding(.horizontal, 12)

            Button("Done") { dismiss() }
                .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
                .frame(width: 120)
        }
        .padding(24)
        .frame(width: 320)
        .background(CRTheme.canvasGradient)
        .onAppear {
            if let ip = getLocalIPAddress() {
                self.localIP = ip
            }
        }
    }

    private func generateQRCode(from string: String) -> NSImage? {
        let data = Data(string.utf8)
        let filter = CIFilter.qrCodeGenerator()
        filter.setValue(data, forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")

        guard let outputImage = filter.outputImage else { return nil }
        
        let context = CIContext()
        guard let cgImage = context.createCGImage(outputImage, from: outputImage.extent) else { return nil }
        
        return NSImage(cgImage: cgImage, size: NSSize(width: 180, height: 180))
    }

    private func HostName() -> String {
        return Host.current().localizedName ?? "Mac"
    }
}

// MARK: - Robust IP Resolver
func getLocalIPAddress() -> String? {
    var address: String?
    var ifaddr: UnsafeMutablePointer<ifaddrs>?
    guard getifaddrs(&ifaddr) == 0 else { return nil }
    guard let firstAddr = ifaddr else { return nil }
    for ptr in sequence(first: firstAddr, next: { $0.pointee.ifa_next }) {
        let interface = ptr.pointee
        let addrFamily = interface.ifa_addr.pointee.sa_family
        if addrFamily == UInt8(AF_INET) {
            let name = String(cString: interface.ifa_name)
            if name.hasPrefix("lo") || name.hasPrefix("pdp_ip") { continue }
            var hostname = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            getnameinfo(interface.ifa_addr, socklen_t(interface.ifa_addr.pointee.sa_len),
                        &hostname, socklen_t(hostname.count),
                        nil, socklen_t(0), NI_NUMERICHOST)
            let ip = String(cString: hostname)
            if !ip.hasPrefix("127.") && !ip.hasPrefix("169.254") {
                address = ip
                if name.hasPrefix("en") {
                    break
                }
            }
        }
    }
    freeifaddrs(ifaddr)
    return address
}
