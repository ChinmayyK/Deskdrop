import SwiftUI
import UniformTypeIdentifiers
import Foundation
import SystemConfiguration
import CoreImage.CIFilterBuiltins

// MARK: - Root

struct DashboardRootView: View {
    @ObservedObject var store: DeskdropStore
    @State private var renameTarget:   ManagedDevice?
    @State private var renameDraft     = ""
    @State private var density: CRDensityMode = .comfortable

    private var pendingContinuityItems: [IpcActivityEntry] {
        store.activityFeed.filter(\.isApplicable)
    }

    var body: some View {
        ZStack(alignment: .bottom) {
            DetailContent(store: store, density: $density, beginRename: beginRename)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            
            FloatingNavBar(store: store)
                .padding(.bottom, 32)
                .zIndex(50)
        }
        .background(CRTheme.surfaceElevated.ignoresSafeArea())
        .ignoresSafeArea(edges: .top)
        .overlay(alignment: .bottomTrailing) {
            if !pendingContinuityItems.isEmpty {
                ContinuityStagingDrawer(entries: Array(pendingContinuityItems.prefix(3)), store: store)
                    .padding(.trailing, 26)
                    .padding(.bottom, 24)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.crSpring, value: pendingContinuityItems.isEmpty)
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

// MARK: - Floating Dock

// MARK: - Application Sidebar

private struct FloatingNavBar: View {
    @ObservedObject var store: DeskdropStore
    @Namespace private var namespace
    
    var body: some View {
        HStack(spacing: 8) {
            ForEach([DashboardSection.devices, .clipboard, .transfers], id: \.self) { section in
                FloatingNavItem(
                    section: section,
                    isSelected: store.selectedSection == section,
                    namespace: namespace
                ) {
                    if store.selectedSection != section {
                        NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)
                        withAnimation(.crSpring) { store.selectedSection = section }
                    }
                }
            }
        }
        .padding(6)
        .background(CRTheme.surfaceElevated.opacity(0.7))
        .background(.ultraThinMaterial)
        .clipShape(Capsule())
        .overlay(
            Capsule()
                .strokeBorder(
                    LinearGradient(
                        colors: [CRTheme.stroke.opacity(0.8), CRTheme.stroke.opacity(0.3)],
                        startPoint: .top, endPoint: .bottom
                    ), 
                    lineWidth: 1
                )
        )
        .shadow(color: .black.opacity(0.12), radius: 24, y: 12)
        .shadow(color: .black.opacity(0.04), radius: 8, y: 4)
    }
}

private struct FloatingNavItem: View {
    let section: DashboardSection
    let isSelected: Bool
    let namespace: Namespace.ID
    let action: () -> Void
    
    @State private var hovered = false
    
    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: isSelected ? section.icon + ".fill" : section.icon)
                    .font(.system(size: 14, weight: isSelected ? .bold : .medium))
                    .symbolRenderingMode(.hierarchical)
                
                if isSelected {
                    Text(section.title)
                        .font(.system(size: 13, weight: .bold))
                }
            }
            .foregroundStyle(isSelected ? .white : (hovered ? CRTheme.ink : CRTheme.inkSoft))
            .padding(.horizontal, isSelected ? 16 : 14)
            .padding(.vertical, 10)
            .background {
                if isSelected {
                    Capsule()
                        .fill(CRTheme.brandElectric)
                        .matchedGeometryEffect(id: "NAV_TAB", in: namespace)
                        .shadow(color: CRTheme.brandElectric.opacity(0.35), radius: 8, y: 3)
                        .overlay(Capsule().strokeBorder(Color.white.opacity(0.15), lineWidth: 1))
                } else if hovered {
                    Capsule()
                        .fill(CRTheme.ink.opacity(0.04))
                }
            }
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.crFast, value: hovered)
    }
}

// MARK: - Detail Content

private struct DetailContent: View {
    @ObservedObject var store: DeskdropStore
    @Binding var density: CRDensityMode
    let beginRename: (ManagedDevice) -> Void

    var body: some View {
        VStack(spacing: 0) {
            // TOP SHELL / APPLICATION CHROME
            ContinuityHeaderView(store: store)
                .zIndex(10)

            // CONTENT REGION
            VStack(spacing: 0) {
                // Content — keyed so SwiftUI rebuilds on section change (enables transition)
                Group {
                    switch store.selectedSection {
                    case .devices: DeviceCentricDashboardView(store: store)
                    case .clipboard: TimelineSectionView(store: store, density: density)
                    case .transfers: TransfersDashboardView(store: store)
                    case .remoteControl: Text("Remote Control Area (Coming Soon)").frame(maxWidth: .infinity, maxHeight: .infinity).foregroundStyle(Color.secondary)
                    case .settings: PreferencesView(store: store)
                    }
                }
                .id(store.selectedSection)
                .transition(.asymmetric(
                    insertion: .opacity.combined(with: .move(edge: .bottom).combined(with: .scale(scale: 0.98))),
                    removal:   .opacity
                ))
                .animation(.crSpring, value: store.selectedSection)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color.clear) // Rely on CRFluidBackgroundView underneath
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder private var toolbarActions: some View {
        switch store.selectedSection {
        case .devices, .clipboard, .transfers, .remoteControl, .settings:
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
        }
    }
}

// MARK: - App Chrome (Top Bar)

private struct ContinuityHeaderView: View {
    @ObservedObject var store: DeskdropStore
    @State private var searchText = ""
    @Environment(\.colorScheme) var scheme

    var body: some View {
        HStack(spacing: 16) {
            // Left Context (Window controls clearance)
            Spacer().frame(width: 72)
            
            Spacer(minLength: 0)
            
            // Command Layer Search (Centered)
            CRSearchField(placeholder: "Search devices, clipboard, files...", text: $searchText)
                .frame(maxWidth: 420)
                
            Spacer(minLength: 16)
            
            // Right Side: Status + Quick Actions
            HStack(spacing: 16) {
                // Network Status
                HStack(spacing: 6) {
                    StatusDot(isOnline: store.connectedCount > 0, size: 8)
                    Text(store.connectedCount > 0 ? "Active" : "Offline")
                        .font(.system(size: 12, weight: .medium, design: .rounded))
                        .foregroundStyle(CRTheme.inkSoft)
                        .fixedSize()
                }
                
                // Quick Actions Pill
                HStack(spacing: 4) {
                    HeaderActionButton(icon: "antenna.radiowaves.left.and.right", tooltip: "Scan Network") {
                        store.scanForDevices()
                    }
                    HeaderActionButton(icon: "paperplane.fill", tooltip: "Send File") {
                        // Triggers file picker
                    }
                    
                    Divider()
                        .frame(height: 16)
                        .padding(.horizontal, 4)
                    
                    HeaderActionButton(icon: "gearshape.fill", tooltip: "Settings") {
                        store.selectedSection = .settings
                    }
                }
                .padding(4)
                .background(Color.black.opacity(0.04), in: Capsule())
                .overlay(Capsule().stroke(CRTheme.stroke.opacity(0.3), lineWidth: 1))
            }
            .layoutPriority(1)
        }
        .padding(.top, 16)
        .padding(.bottom, 12)
        .background {
            ZStack {
                CRVisualEffect(material: .headerView, blendingMode: .withinWindow)
                
                // Subtle bottom border
                VStack {
                    Spacer()
                    Rectangle()
                        .fill(CRTheme.stroke.opacity(0.5))
                        .frame(height: 1)
                }
                
                // Ambient top glow
                VStack {
                    Rectangle()
                        .fill(LinearGradient(colors: [Color.white.opacity(scheme == .dark ? 0.05 : 0.4), .clear], startPoint: .top, endPoint: .bottom))
                        .frame(height: 12)
                    Spacer()
                }
            }
        }
    }
}

private struct HeaderActionButton: View {
    let icon: String
    let tooltip: String
    let action: () -> Void
    @State private var isHovered = false
    
    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(isHovered ? CRTheme.brandElectric : CRTheme.inkSoft)
                .frame(width: 32, height: 32)
                .background(
                    Circle().fill(isHovered ? CRTheme.brandElectric.opacity(0.1) : Color.clear)
                )
        }
        .buttonStyle(.plain)
        .help(tooltip)
        .crHoverScale(scale: 1.1)
        .onHover { isHovered = $0 }
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
                    Text(device.connectionState == .connected ? "Clipboard Ready" : device.connectionState.label.capitalized)
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
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(CRTheme.surfaceStrong)
                .overlay {
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .strokeBorder(CRTheme.brandElectric.opacity(0.50), lineWidth: 1)
                }
                .shadow(color: Color.black.opacity(0.15), radius: 8, y: 4)
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
    @ObservedObject var store: DeskdropStore

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
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(CRTheme.surfaceStrong)
                    .overlay {
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .strokeBorder(CRTheme.stroke, lineWidth: 1)
                    }
                    .shadow(color: Color.black.opacity(0.15), radius: 8, y: 4)
            }
            .overlay(alignment: .bottomLeading) {
                // Animated shrinking progress line
                Rectangle()
                    .fill(CRTheme.brandElectric)
                    .frame(height: 3)
                    .frame(maxWidth: progress * 336, alignment: .leading)
                    .clipShape(RoundedRectangle(cornerRadius: 1.5))
                    .padding(.horizontal, 16)
                    .padding(.bottom, 6)
            }
            .onAppear {
                progress = 1.0
                withAnimation(.linear(duration: 5.0)) {
                    progress = 0.0
                }
            }
        }
    }
    
    @State private var progress: CGFloat = 1.0

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
    @ObservedObject var store: DeskdropStore
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
                            LazyVGrid(columns: [GridItem(.flexible(), spacing: 16), GridItem(.flexible(), spacing: 16)], spacing: 16) {
                                ForEach(filteredItems) { item in
                                    TimelineCard(item: item, store: store, density: density)
                                        .modifier(MasonryGridModifier())
                                        .transition(.scale(scale: 0.95).combined(with: .opacity))
                                        .animation(.crSpring, value: filteredItems.count)
                                }
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

// MARK: - Spatial Modifiers

private struct MasonryGridModifier: ViewModifier {
    func body(content: Content) -> some View {
        GeometryReader { proxy in
            let y = proxy.frame(in: .global).minY
            // Approximating screen height for 3D tilt calculations
            let screenH = NSScreen.main?.frame.height ?? 1000
            let normalizedY = (y / screenH)
            
            content
                .rotation3DEffect(
                    .degrees(Double((normalizedY - 0.5) * 6)),
                    axis: (x: 1, y: 0, z: 0),
                    perspective: 0.8
                )
                .scaleEffect(1.0 - (abs(normalizedY - 0.5) * 0.03))
                .opacity(normalizedY > 0.9 ? 0.4 : 1.0)
        }
        // Fixed minimum height to prevent the geometry reader from collapsing
        .frame(minHeight: 100) 
    }
}

// MARK: - Devices Section

private struct DevicesSectionView: View {
    @ObservedObject var store: DeskdropStore
    let rename: (ManagedDevice) -> Void
    @State private var showingFileImporter = false
    @State private var pendingFileTarget:  ManagedDevice?

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 14) {
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
                    LazyVStack(spacing: 7) {
                        ForEach(store.devices) { DeviceCard(device: $0, store: store, rename: rename) }
                    }
                }
            }
            .padding(.horizontal, 20).padding(.bottom, 24)
        }
        .fileImporter(isPresented: $showingFileImporter, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            if case let .success(urls) = result {
                store.sendFiles(urls: urls, to: pendingFileTarget)
                pendingFileTarget = nil
            }
        }
    }
}

// MARK: - Trust Section

private struct TrustSectionView: View {
    @ObservedObject var store: DeskdropStore
    let rename: (ManagedDevice) -> Void
    private var attention: [ManagedDevice] { store.devices.filter { $0.trustState != .trusted } }

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 8) {
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
                    LazyVStack(spacing: 7) {
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
    @ObservedObject var store: DeskdropStore
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
        .crCard(cornerRadius: density.cardRadius, highlighted: isHovered, accent: accent)
        .onHover { isHovered = $0 }
        .animation(.crFast, value: isHovered)
    }
}

// MARK: - Device Card

private struct DeviceCard: View {
    let device: ManagedDevice
    @ObservedObject var store: DeskdropStore
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
                    if device.pairingRequested {
                        Button("Accept") { store.respondToPairing(device, accepted: true) }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
                        Button("Decline") { store.respondToPairing(device, accepted: false) }.buttonStyle(CRDestructiveButtonStyle())
                    } else {
                        Button("Pair") { store.sendPairingRequest(device) }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
                        Button("Trust (Legacy)")  { store.trust(device) }.buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
                        Button("Reject (Legacy)") { store.reject(device) }.buttonStyle(CRDestructiveButtonStyle())
                    }
                } else {
                    Button("Rename")       { rename(device) }.buttonStyle(CRSecondaryButtonStyle())
                    Button("Revoke Trust") { store.revoke(device) }.buttonStyle(CRDestructiveButtonStyle())
                    Button("Forget")       { Task { try? await DeskdropIPCClient.shared.forgetDevice(deviceId: device.id); store.scanForDevices() } }.buttonStyle(CRDestructiveButtonStyle())
                }
                Spacer()
            }
            .padding(.horizontal, 14).padding(.vertical, 9)
        }
        .crCard(cornerRadius: 8, highlighted: isHovered, accent: accent)
        .onHover { isHovered = $0 }
        .animation(.crFast, value: isHovered)
    }
}

// MARK: - Manual Connect Card

private struct ManualConnectCard: View {
    @ObservedObject var store: DeskdropStore
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
        .crCard(cornerRadius: 8, highlighted: hovered)
        .onHover { hovered = $0 }.animation(.crFast, value: hovered)
    }
}

// MARK: - File Share Card

private struct FileShareCard: View {
    @ObservedObject var store: DeskdropStore
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

// MARK: - Device-Centric Dashboard (O+ Connect Style)

struct DeviceCentricDashboardView: View {
    @ObservedObject var store: DeskdropStore
    @State private var showingFilePicker = false
    @State private var pendingFileTarget: ManagedDevice?
    @Environment(\.colorScheme) var scheme

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                // Header
                HStack {
                    Text("Welcome back, \(NSUserName().capitalized)!")
                        .font(.system(size: 28, weight: .semibold, design: .rounded))
                        .foregroundStyle(CRTheme.ink)
                    Spacer()
                }
                .padding(.horizontal, 40)
                .padding(.top, 40)
                .padding(.bottom, 20)

                if !store.connectedDevices.isEmpty {
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 16), GridItem(.flexible(), spacing: 16)], spacing: 16) {
                        ForEach(store.connectedDevices, id: \.id) { device in
                            CompactDeviceCard(device: device, store: store) {
                                pendingFileTarget = device
                                showingFilePicker = true
                            }
                        }
                    }
                    .padding(.horizontal, 40)
                } else {
                    CompactEmptyState(store: store)
                        .padding(.horizontal, 40)
                }
                
                // Quick Actions
                VStack(spacing: 14) {
                    MagicLinkPairingCard(store: store)
                    ManualConnectCard(store: store)
                    FileShareCard(store: store) { pendingFileTarget = $0; showingFilePicker = true }
                }
                .padding(.horizontal, 40)
                .padding(.top, 32)
                .padding(.bottom, 40)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.clear)
        .fileImporter(isPresented: $showingFilePicker, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result {
                store.sendFiles(urls: urls, to: pendingFileTarget)
                pendingFileTarget = nil
            }
        }
    }
}

// MARK: - Hero Device Layouts

private struct CompactEmptyState: View {
    @ObservedObject var store: DeskdropStore

    var body: some View {
        VStack(spacing: 16) {
            ZStack {
                Circle().fill(CRTheme.brandElectric.opacity(0.1)).frame(width: 48, height: 48)
                Image(systemName: "macbook.and.iphone").foregroundStyle(CRTheme.brandElectric).font(.system(size: 24))
            }
            Text("No devices connected")
                .font(.system(size: 14, weight: .medium, design: .rounded))
                .foregroundStyle(CRTheme.ink)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
        .background(CRTheme.surfaceElevated.opacity(0.5))
        .crCard(cornerRadius: 16)
    }
}

private struct CompactDeviceCard: View {
    let device: ManagedDevice
    @ObservedObject var store: DeskdropStore
    let onSendFiles: () -> Void
    @Environment(\.colorScheme) var colorScheme
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 16) {
            // Icon
            ZStack {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(LinearGradient(colors: [CRTheme.brandElectric, CRTheme.brandViolet], startPoint: .topLeading, endPoint: .bottomTrailing))
                    .frame(width: 48, height: 48)
                if device.name.lowercased().contains("mac") {
                    Image(systemName: "laptopcomputer")
                        .font(.system(size: 20))
                        .foregroundStyle(.white)
                } else if let imgPath = Bundle.main.path(forResource: "AndroidLogo", ofType: "png"), let nsImg = NSImage(contentsOfFile: imgPath) {
                    Image(nsImage: nsImg)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 24, height: 24)
                } else {
                    Image(systemName: "smartphone")
                        .font(.system(size: 20))
                        .foregroundStyle(.white)
                }
            }
            
            // Text
            VStack(alignment: .leading, spacing: 4) {
                Text(device.name)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(CRTheme.ink)
                    .lineLimit(1)
                
                HStack(spacing: 8) {
                    HStack(spacing: 4) {
                        Circle().fill(CRTheme.accentGreen).frame(width: 6, height: 6)
                        Text("Connected")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(CRTheme.accentGreen)
                    }
                    if let battery = store.peerBatteries.first(where: { $0.deviceId == device.id }) {
                        BatteryIndicatorPill(level: battery.level, charging: battery.charging)
                    }
                }
            }
            
            Spacer(minLength: 8)
            
            // Actions
            HStack(spacing: 8) {
                Button(action: onSendFiles) {
                    HStack(spacing: 4) {
                        Image(systemName: "folder")
                            .font(.system(size: 13, weight: .semibold))
                        Text("Files")
                            .font(.system(size: 12, weight: .medium))
                    }
                    .foregroundStyle(CRTheme.ink)
                }
                .buttonStyle(CRSecondaryButtonStyle())
                .help("Send Files")
                
                Button(action: { store.disconnect(device) }) {
                    Image(systemName: "link.badge.minus")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(CRTheme.accentRed)
                }
                .buttonStyle(CRSecondaryButtonStyle())
                .help("Disconnect")
            }
        }
        .padding(16)
        .background(CRTheme.surfaceElevated)
        .crCard(cornerRadius: 16)
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(CRTheme.brandElectric.opacity(isHovered ? 0.3 : 0.0), lineWidth: 1)
        }
        .onHover { isHovered = $0 }
        .animation(.crSpring, value: isHovered)
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
    @ObservedObject var store: DeskdropStore
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
    @ObservedObject var store: DeskdropStore
    @Environment(\.dismiss) var dismiss
    @State private var localIP: String = "127.0.0.1"

    var pairingURL: String {
        let name = store.settings?.deviceName ?? HostName()
        let port = store.settings?.port ?? 47823
        let fp = store.localFingerprint ?? ""
        return "deskdrop://pair?name=\(name.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? "")&ip=\(localIP)&port=\(port)&fingerprint=\(fp)"
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

            Text("Scan the QR code below from the Deskdrop Android app to pair instantly, or use the 6-digit confirmation PIN.")
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
        .onChange(of: store.connectedDevices.count) { _ in
            if !store.connectedDevices.isEmpty {
                dismiss()
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

// MARK: - New Ecosystem UI Components

struct DeviceIdentityCard: View {
    let device: ManagedDevice
    var isOnline: Bool = true
    @State private var isHovered = false
    
    // Platform distinct identity
    private var platformBrand: Color {
        let name = device.name.lowercased()
        if name.contains("mac") { return CRTheme.ink } // Silver/Dark
        if name.contains("android") { return CRTheme.accentGreen } // Android green
        if name.contains("windows") { return CRTheme.accentBlue } // Windows blue
        return CRTheme.accentIndigo
    }
    
    var body: some View {
        HStack(spacing: 14) {
            ZStack(alignment: .bottomTrailing) {
                // Outer glow ring
                Circle()
                    .stroke(platformBrand.opacity(isOnline ? 0.3 : 0.0), lineWidth: 2)
                    .frame(width: 46, height: 46)
                    .overlay(
                        Circle()
                            .trim(from: 0.0, to: 0.8) // Mock 80% battery
                            .stroke(platformBrand, style: StrokeStyle(lineWidth: 2, lineCap: .round))
                            .rotationEffect(.degrees(-90))
                            .opacity(isOnline ? 1.0 : 0.0)
                    )
                
                DeviceAvatar(name: device.name, platform: nil, size: 38, color: platformBrand)
                
                if isOnline {
                    StatusDot(isOnline: true, size: 10)
                        .overlay(Circle().stroke(CRTheme.surfaceElevated, lineWidth: 2))
                        .offset(x: 2, y: 2)
                }
            }
            
            VStack(alignment: .leading, spacing: 3) {
                Text(device.name)
                    .font(.system(size: 14, weight: .bold, design: .rounded))
                    .foregroundStyle(isOnline ? CRTheme.ink : CRTheme.inkSoft)
                
                HStack(spacing: 6) {
                    if device.name.lowercased().contains("mac") {
                        Image(systemName: "macbook.and.iphone")
                            .font(.system(size: 11))
                    } else if let imgPath = Bundle.main.path(forResource: "AndroidLogo", ofType: "png"), let nsImg = NSImage(contentsOfFile: imgPath) {
                        Image(nsImage: nsImg)
                            .resizable()
                            .scaledToFit()
                            .frame(width: 12, height: 12)
                    } else {
                        Image(systemName: "smartphone")
                            .font(.system(size: 11))
                    }
                    
                    Text(isOnline ? "Active · 23ms" : "Offline") // Mock ping
                        .font(.system(size: 11, weight: .semibold))
                }
                .foregroundStyle(isOnline ? platformBrand : CRTheme.inkSubtle)
            }
            .padding(.trailing, 10)
            
            if isOnline {
                // Live ping waves
                HStack(spacing: 2) {
                    ForEach(0..<3) { i in
                        Capsule()
                            .fill(platformBrand.opacity(0.4))
                            .frame(width: 2, height: CGFloat.random(in: 4...12))
                    }
                }
                .padding(.leading, 8)
                .animation(.easeInOut(duration: 0.5).repeatForever(), value: isHovered)
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(CRTheme.surfaceElevated.opacity(isHovered ? 1.0 : 0.6))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16)
                .strokeBorder(isHovered ? platformBrand.opacity(0.3) : CRTheme.stroke.opacity(0.5), lineWidth: 1)
        )
        .opacity(isOnline ? 1.0 : 0.5)
        .shadow(color: isHovered ? platformBrand.opacity(0.15) : Color.clear, radius: 8, y: 4)
        .crHoverScale(scale: isOnline ? 1.04 : 1.0)
        .onHover { isHovered = $0 }
    }
}

struct PushClipboardFeaturedCard: View {
    let action: () -> Void
    @State private var isHovered = false
    @ObservedObject var store: DeskdropStore // Need store for avatars and preview
    
    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Push Clipboard")
                            .font(.system(size: 24, weight: .semibold, design: .rounded))
                            .foregroundStyle(CRTheme.ink)
                        Text("Sync copied text instantly")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                    Spacer()
                    ZStack {
                        Circle().fill(CRTheme.accentBlue.opacity(0.15)).frame(width: 48, height: 48)
                        Image(systemName: "doc.on.clipboard.fill")
                            .font(.system(size: 22, weight: .medium))
                            .foregroundStyle(CRTheme.accentBlue)
                        
                        // Sync ripple
                        if isHovered {
                            Circle()
                                .stroke(CRTheme.brandElectric.opacity(0.5), lineWidth: 1)
                                .frame(width: 48, height: 48)
                                .scaleEffect(1.5)
                                .opacity(0.0)
                                .animation(.easeOut(duration: 1.0).repeatForever(autoreverses: false), value: isHovered)
                        }
                    }
                }
                
                Spacer(minLength: 24)
                
                VStack(alignment: .leading, spacing: 14) {
                    // Live Clipboard Preview
                    if let lastText = store.timeline.first(where: { $0.typeLabel == "Text" })?.fullText {
                        Text("\"\(lastText.prefix(40))...\"")
                            .font(.system(size: 13, weight: .medium, design: .monospaced))
                            .foregroundStyle(CRTheme.brandElectric)
                            .lineLimit(1)
                            .padding(.vertical, 8)
                            .padding(.horizontal, 12)
                            .background(CRTheme.brandElectric.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
                    } else {
                        Text("Clipboard ready...")
                            .font(.system(size: 13, weight: .medium, design: .monospaced))
                            .foregroundStyle(CRTheme.inkSubtle)
                            .padding(.vertical, 8)
                            .padding(.horizontal, 12)
                            .background(CRTheme.inkSubtle.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
                    }
                    
                    // Connected Targets & Live Status
                    HStack(alignment: .center) {
                        Text("Synced to:")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(CRTheme.inkSoft)
                        
                        if !store.connectedDevices.isEmpty {
                            HStack(spacing: -6) {
                                ForEach(Array(store.connectedDevices.prefix(3))) { d in
                                    DeviceAvatar(name: d.name, platform: nil, size: 20, color: CRTheme.accentBlue)
                                        .overlay(Circle().stroke(CRTheme.surfaceElevated, lineWidth: 2))
                                }
                            }
                        } else {
                            Text("No devices")
                                .font(.system(size: 12, weight: .medium))
                                .foregroundStyle(CRTheme.inkSubtle)
                        }
                        
                        Spacer()
                        
                        // Tiny waveform animation
                        HStack(spacing: 2) {
                            ForEach(0..<4) { i in
                                Capsule()
                                    .fill(CRTheme.brandElectric.opacity(0.6))
                                    .frame(width: 2, height: isHovered ? CGFloat.random(in: 4...12) : 4)
                            }
                        }
                        .animation(.easeInOut(duration: 0.3).repeatForever(), value: isHovered)
                        
                        Text("Live now")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(CRTheme.accentBlue)
                    }
                }
            }
            .padding(24)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .crCard(cornerRadius: 24, highlighted: isHovered, accent: CRTheme.accentBlue)
            .crHoverScale(scale: 1.02)
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

struct SecondaryActionCard: View {
    let icon: String
    let title: String
    let color: Color
    let action: () -> Void
    @State private var isHovered = false
    
    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 12) {
                ZStack {
                    Circle().fill(color.opacity(0.15)).frame(width: 36, height: 36)
                    Image(systemName: icon)
                        .font(.system(size: 16, weight: .medium))
                        .foregroundStyle(color)
                }
                
                Text(title)
                    .font(.system(size: 15, weight: .semibold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .crCard(cornerRadius: 16, highlighted: isHovered, accent: color)
            .crHoverScale(scale: 1.03)
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}

// MARK: - Legacy Action Cards

struct QuickActionCard: View {
    let icon: String
    let title: String
    let subtitle: String
    var isDragTarget: Bool = false
    let action: () -> Void
    
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(CRTheme.brandElectric.opacity(isDragTarget ? 0.25 : 0.12))
                        .frame(width: 32, height: 32)
                    Image(systemName: icon)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(CRTheme.brandElectric)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(Color.primary)
                    Text(subtitle)
                        .font(.system(size: 11))
                        .foregroundStyle(Color.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(Color(NSColor.controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .shadow(color: .black.opacity(0.05), radius: 2, y: 1)
            .overlay {
                if isDragTarget || hovered {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(CRTheme.brandElectric.opacity(0.4), lineWidth: 1)
                }
            }
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.crFast, value: isDragTarget)
        .animation(.crFast, value: hovered)
    }
}

struct DeviceListRow: View {
    let device: ManagedDevice
    @ObservedObject var store: DeskdropStore
    @State private var hovered = false
    
    var body: some View {
        HStack(spacing: 16) {
            // Leading: Device type icon
            Image(systemName: device.name.lowercased().contains("mac") ? "laptopcomputer" : "smartphone")
                .font(.system(size: 18, weight: .regular))
                .foregroundStyle(CRTheme.brandElectric)
                .frame(width: 24, alignment: .center)
            
            // Center: Device Name and status
            VStack(alignment: .leading, spacing: 2) {
                Text(device.name)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(Color.primary)
                HStack {
                    Text(device.lastSync?.relativeTimeString() ?? "Synced just now")
                        .font(.system(size: 11.5))
                        .foregroundStyle(Color.secondary)

                }
            }
            Spacer()
            
            // Trailing: Icon-only push button
            Button {
                store.sendCurrentClipboard(to: device)
            } label: {
                Image(systemName: "arrow.up.doc.fill")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(hovered ? CRTheme.brandElectric : Color.secondary)
                    .frame(width: 32, height: 32)
                    .background(Color(NSColor.controlBackgroundColor), in: Circle())
                    .shadow(color: .black.opacity(0.05), radius: 1, y: 1)
            }
            .buttonStyle(.plain)
            .onHover { hovered = $0 }
            .help("Push clipboard to \(device.name)")
        }
        .padding(.horizontal, 28)
        .padding(.vertical, 14)
    }
}

// MARK: - Continuity Orb

struct FloatingContinuityOrb: View {
    @State private var phase = 0.0
    var activeCount: Int
    
    var body: some View {
        ZStack {
            // Core orb
            Circle()
                .fill(CRTheme.brandCyan.opacity(0.15))
                .frame(width: 60, height: 60)
                .blur(radius: 8)
                .scaleEffect(1.0 + phase * 0.1)
            
            Circle()
                .stroke(CRTheme.brandCyan.opacity(0.4), lineWidth: 1)
                .frame(width: 60, height: 60)
            
            Image(systemName: "network")
                .font(.system(size: 24, weight: .light))
                .foregroundStyle(CRTheme.brandCyan)
            
            // Orbiting particles
            if activeCount > 0 {
                ForEach(0..<activeCount, id: \.self) { i in
                    Circle()
                        .fill(CRTheme.brandElectric)
                        .frame(width: 6, height: 6)
                        .offset(x: 40)
                        .rotationEffect(.degrees(Double(i) * (360.0 / Double(activeCount)) + phase * 360))
                }
            }
        }
        .onAppear {
            withAnimation(.linear(duration: 8).repeatForever(autoreverses: false)) {
                phase = 1.0
            }
        }
    }
}
