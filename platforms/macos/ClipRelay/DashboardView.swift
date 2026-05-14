import SwiftUI
import UniformTypeIdentifiers

// MARK: - Root

struct DashboardRootView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var renameTarget:   ManagedDevice?
    @State private var renameDraft     = ""
    @State private var density: CRDensityMode = .comfortable
    @State private var columnVisibility: NavigationSplitViewVisibility = .all

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            Sidebar(store: store)
                .navigationSplitViewColumnWidth(min: 210, ideal: 226, max: 250)
        } detail: {
            DetailContent(store: store, density: $density, beginRename: beginRename)
        }
        .overlay(alignment: .topTrailing) {
            CRToastStack(toasts: store.toasts).padding(18)
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

            // Subtle top shimmer
            VStack {
                LinearGradient(colors: [Color(white: 1, opacity: 0.028), .clear],
                               startPoint: .top, endPoint: .bottom)
                    .frame(height: 80)
                Spacer()
            }

            VStack(alignment: .leading, spacing: 0) {
                SidebarHeader(store: store)
                CRDividerDark().padding(.horizontal, 16).padding(.vertical, 12)

                Text("NAVIGATION")
                    .font(.system(size: 9.5, weight: .bold)).tracking(1.2)
                    .foregroundStyle(Color(white: 1, opacity: 0.20))
                    .padding(.horizontal, 18).padding(.bottom, 4)

                VStack(spacing: 1) {
                    SidebarNavButton(icon: "clock.arrow.circlepath", label: "Timeline",
                                     badge: 0, shortcut: "⌘1",
                                     isSelected: store.selectedSection == .timeline,
                                     action: { store.selectedSection = .timeline })
                    SidebarNavButton(icon: "rectangle.connected.to.line.below", label: "Devices",
                                     badge: store.devices.count, shortcut: "⌘2",
                                     isSelected: store.selectedSection == .devices,
                                     action: { store.selectedSection = .devices })
                    SidebarNavButton(icon: "shield.checkered", label: "Trust",
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
                        .foregroundStyle(.white)
                    HStack(spacing: 4) {
                        StatusDot(isOnline: store.isRunning, size: 5.5)
                        Text(store.isRunning ? "Active" : "Offline")
                            .font(.system(size: 10.5, weight: .medium))
                            .foregroundStyle(Color(white: 1, opacity: 0.40))
                    }
                }
                Spacer()
            }
            if !store.connectionBanner.isEmpty {
                HStack(spacing: 5) {
                    Image(systemName: "wifi").font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(Color(white: 1, opacity: 0.24))
                    Text(store.connectionBanner).font(.system(size: 11))
                        .foregroundStyle(Color(white: 1, opacity: 0.32))
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
                Text("Local-first").font(.system(size: 10)).foregroundStyle(Color(white: 1, opacity: 0.24))
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
                                message: search.isEmpty ? "Try a different filter." : "No items match "\(search)".",
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
                        message: "When another ClipRelay device appears on your network it will show up here.",
                        accent: CRTheme.inkSoft,
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
            if case let .success(urls) = result, let url = urls.first {
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
            CRIconChip(systemName: "arrow.up.doc.fill", tint: CRTheme.accentIndigo, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text("Send a File").font(.system(size: 13, weight: .semibold)).foregroundStyle(CRTheme.ink)
                Text("Push a document, image, or archive to peers").font(.system(size: 11.5)).foregroundStyle(CRTheme.inkSoft)
            }
            Spacer()
            Button("Send to all") { chooseTarget(nil) }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentIndigo))
            if !store.connectedDevices.isEmpty {
                Menu("Choose…") {
                    ForEach(store.connectedDevices) { d in Button(d.name) { chooseTarget(d) } }
                }
                .buttonStyle(CRSecondaryButtonStyle())
            }
        }
        .padding(14).frame(maxWidth: .infinity)
        .crCard(cornerRadius: 11, highlighted: hovered, accent: CRTheme.accentIndigo)
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
