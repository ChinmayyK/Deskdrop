import SwiftUI
import UniformTypeIdentifiers

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
        .overlay(alignment: .topTrailing) {
            if let device = store.connectedDevices.first, store.selectedSection != .settings {
                CompanionDeviceCard(device: device, connectedPeers: store.connectedDevices.count)
                    .padding(.top, 26)
                    .padding(.trailing, 26)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
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
    private var untrustedCount: Int { store.devices.filter { $0.trustState == .untrusted }.count }

    var body: some View {
        ZStack {
            CRSidebarMaterial().ignoresSafeArea()
            CRTheme.sidebarOverlay.ignoresSafeArea()

            // Subtle top shimmer (adaptive: just-visible in light, barely-there in dark)
            VStack {
                LinearGradient(
                    colors: [CRTheme.sidebarInk.opacity(0.018), .clear],
                    startPoint: .top, endPoint: .bottom)
                    .frame(height: 80)
                Spacer()
            }

            VStack(alignment: .leading, spacing: 0) {
                SidebarHeader(store: store)
                CRDividerDark().padding(.horizontal, 16).padding(.vertical, 12)

                Text("NAVIGATION")
                    .font(.system(size: 9.5, weight: .bold)).tracking(1.2)
                    .foregroundStyle(CRTheme.sidebarInkSubtle)
                    .padding(.horizontal, 18).padding(.bottom, 4)

                VStack(spacing: 1) {
                    SidebarNavButton(icon: "clock.arrow.circlepath", label: "Timeline",
                                     badge: 0, shortcut: "⌘1",
                                     isSelected: store.selectedSection == .timeline,
                                     action: { store.selectedSection = .timeline })
                    SidebarNavButton(icon: "desktopcomputer", label: "Devices",
                                     badge: store.devices.count, shortcut: "⌘2",
                                     isSelected: store.selectedSection == .devices,
                                     action: { store.selectedSection = .devices })
                    SidebarNavButton(icon: "checkmark.shield", label: "Trust",
                                     badge: untrustedCount, shortcut: "⌘3",
                                     isSelected: store.selectedSection == .trust,
                                     action: { store.selectedSection = .trust })
                    SidebarNavButton(icon: "slider.horizontal.3", label: "Settings",
                                     badge: 0, shortcut: "⌘4",
                                     isSelected: store.selectedSection == .settings,
                                     action: { store.selectedSection = .settings })
                }
                .padding(.horizontal, 8)

                Spacer()
                SidebarFooter(store: store)
            }
        }
        // Keyboard shortcuts ⌘1-4
        .background {
            Group {
                Button("") { store.selectedSection = .timeline }.keyboardShortcut("1", modifiers: .command)
                Button("") { store.selectedSection = .devices  }.keyboardShortcut("2", modifiers: .command)
                Button("") { store.selectedSection = .trust    }.keyboardShortcut("3", modifiers: .command)
                Button("") { store.selectedSection = .settings }.keyboardShortcut("4", modifiers: .command)
            }
            .frame(width: 0, height: 0).opacity(0)
        }
    }
}

// MARK: - Sidebar Header

private struct SidebarHeader: View {
    @ObservedObject var store: ClipRelayStore
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                CRAppIconMark(size: 34)
                VStack(alignment: .leading, spacing: 2) {
                    Text("ClipRelay")
                        .font(.system(size: 14.5, weight: .bold, design: .rounded))
                        .foregroundStyle(CRTheme.sidebarInk)
                    HStack(spacing: 4) {
                        StatusDot(isOnline: store.isRunning, size: 5.5)
                        Text(store.isRunning ? "Active" : "Offline")
                            .font(.system(size: 10.5, weight: .medium))
                            .foregroundStyle(CRTheme.sidebarInkSoft)
                    }
                }
                Spacer()
            }
            if !store.connectionBanner.isEmpty {
                HStack(spacing: 5) {
                    Image(systemName: "wifi").font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(CRTheme.sidebarInkSubtle)
                    Text(store.connectionBanner).font(.system(size: 11))
                        .foregroundStyle(CRTheme.sidebarInkSoft)
                        .lineLimit(1).truncationMode(.tail)
                }
            }
        }
        .padding(.horizontal, 16).padding(.top, 20).padding(.bottom, 4)
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
                case .timeline: TimelineSectionView(store: store, density: density)
                case .devices:  DevicesSectionView(store: store, rename: beginRename)
                case .trust:    TrustSectionView(store: store, rename: beginRename)
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
        case .timeline:
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

        case .trust:
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
                .fill(Color.white.opacity(0.72))
                .overlay {
                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                        .strokeBorder(CRTheme.stroke.opacity(0.36), lineWidth: 0.5)
                }
                .shadow(color: .black.opacity(0.06), radius: 18, x: 0, y: 8)
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
                            Button("Copy") { store.copyTimelineItem(item) }
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
