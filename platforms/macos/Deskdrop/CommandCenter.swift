import SwiftUI

struct CommandCenterRootView: View {
    @ObservedObject var store: DeskdropStore
    @State private var renameTarget: ManagedDevice?
    @State private var renameDraft = ""
    @State private var density: CRDensityMode = .comfortable

    private var pendingContinuityItems: [IpcActivityEntry] {
        store.activityFeed.filter(\.isApplicable)
    }

    var body: some View {
        HStack(spacing: 0) {
            // Left Column: Navigation Sidebar (240px)
            CommandSidebarView(store: store)
                .frame(width: 240)
                .background(.regularMaterial)
            
            Divider().opacity(0.5)
            
            // Center Column: Main Workspace (Flexible)
            ZStack(alignment: .bottom) {
                // Content Router
                Group {
                    switch store.selectedSection {
                    case .devices: 
                        CommandCenterView(store: store)
                    case .clipboard: 
                        TimelineSectionView(store: store, density: density)
                    case .transfers: 
                        TransfersDashboardView(store: store)
                    case .remoteControl: 
                        Text("Remote Control Area (Coming Soon)").foregroundStyle(Color.secondary)
                    case .settings: 
                        PreferencesView(store: store)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .id(store.selectedSection)
                .transition(.opacity)
                .animation(.crSpring, value: store.selectedSection)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(CRTheme.surface)
            
            Divider().opacity(0.5)
            
            // Right Column: Smart Device Panel (320px)
            LiveDevicePanel(store: store)
                .frame(width: 320)
                .background(CRTheme.surfaceElevated)
        }
        .ignoresSafeArea(.all, edges: .top)
        .frame(minWidth: 1100, minHeight: 700)
        .background(CRTheme.surfaceStrong.ignoresSafeArea())
        .sheet(item: $renameTarget) { device in
            // Fallback for store requirements
            Text("Rename \(device.name)")
        }
    }
}

// MARK: - Left Sidebar
struct CommandSidebarView: View {
    @ObservedObject var store: DeskdropStore
    @State private var hoveredSection: DashboardSection? = nil
    
    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            // App Branding removed
            // Main Navigation
            VStack(alignment: .leading, spacing: 6) {
                Text("WORKSPACE")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(CRTheme.inkSubtle)
                    .padding(.horizontal, 16)
                    .padding(.top, 24)
                    .padding(.bottom, 4)
                
                ForEach([DashboardSection.devices, .clipboard, .transfers, .settings], id: \.self) { section in
                    SidebarNavItem(
                        section: section, 
                        isSelected: store.selectedSection == section,
                        action: { withAnimation(.crSpring) { store.selectedSection = section } }
                    )
                }
            }
            
            // Connected Devices List
            VStack(alignment: .leading, spacing: 6) {
                Text("DEVICES")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(CRTheme.inkSubtle)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 4)
                
                if store.connectedDevices.isEmpty {
                    Text("No active devices")
                        .font(.system(size: 13))
                        .foregroundStyle(CRTheme.inkSoft)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                } else {
                    ForEach(store.connectedDevices) { device in
                        Button(action: {
                            withAnimation(.crSpring) {
                                store.selectedSection = .devices
                                store.selectedPendingDevice = nil
                            }
                        }) {
                            HStack(spacing: 10) {
                                Circle()
                                    .fill(CRTheme.accentGreen)
                                    .frame(width: 8, height: 8)
                                Text(device.name)
                                    .font(.system(size: 13, weight: (store.selectedSection == .devices && store.selectedPendingDevice == nil) ? .bold : .medium))
                                Spacer()
                            }
                            .foregroundStyle((store.selectedSection == .devices && store.selectedPendingDevice == nil) ? Color.white : CRTheme.ink)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(
                                RoundedRectangle(cornerRadius: 8, style: .continuous)
                                    .fill((store.selectedSection == .devices && store.selectedPendingDevice == nil) ? CRTheme.brandElectric : Color.clear)
                            )
                            .padding(.horizontal, 12)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            
            if !store.pendingDevices.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("PENDING")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(CRTheme.inkSubtle)
                        .padding(.horizontal, 16)
                        .padding(.bottom, 4)
                    
                    ForEach(store.pendingDevices) { device in
                        Button(action: {
                            withAnimation(.crSpring) {
                                store.selectedSection = .devices
                                store.selectedPendingDevice = device
                            }
                        }) {
                            HStack(spacing: 10) {
                                Circle()
                                    .fill(CRTheme.accentOrange)
                                    .frame(width: 8, height: 8)
                                Text(device.name)
                                    .font(.system(size: 13, weight: store.selectedPendingDevice?.id == device.id ? .bold : .medium))
                                Spacer()
                            }
                            .foregroundStyle(store.selectedPendingDevice?.id == device.id ? Color.white : CRTheme.ink)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(
                                RoundedRectangle(cornerRadius: 8, style: .continuous)
                                    .fill(store.selectedPendingDevice?.id == device.id ? CRTheme.brandElectric : Color.clear)
                            )
                            .padding(.horizontal, 12)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            
            Spacer()
        }
    }
}

struct SidebarNavItem: View {
    let section: DashboardSection
    let isSelected: Bool
    let action: () -> Void
    @State private var hovered = false
    
    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: section.icon)
                    .font(.system(size: 14, weight: isSelected ? .bold : .medium))
                    .frame(width: 20)
                Text(section.title)
                    .font(.system(size: 13, weight: isSelected ? .bold : .medium))
                Spacer()
            }
            .foregroundStyle(isSelected ? Color.white : (hovered ? CRTheme.ink : CRTheme.inkSoft))
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background {
                if isSelected {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(CRTheme.brandElectric)
                } else if hovered {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(CRTheme.ink.opacity(0.04))
                }
            }
            .padding(.horizontal, 12)
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.crFast, value: hovered)
    }
}

// MARK: - Center Column
struct CommandCenterView: View {
    @ObservedObject var store: DeskdropStore
    @State private var searchQuery = ""
    @State private var showingFilePicker = false
    @State private var pendingFileTarget: ManagedDevice?
    @State private var showingRemoteExplorer = false

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 20) {
                
                // Global Search Placeholder
                HStack {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(CRTheme.inkSubtle)
                    TextField("Search Files, Clipboard, Devices...", text: $searchQuery)
                        .textFieldStyle(.plain)
                        .font(.system(size: 15))
                }
                .padding(14)
                .background(CRTheme.surfaceStrong)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(CRTheme.stroke, lineWidth: 1))
                .padding(.horizontal, 40)
                .padding(.top, 16)
                
                // Dynamic Hero Header
                DynamicHeroHeaderView(store: store)
                
                // 2. Launchpad (Quick Actions)
                VStack(alignment: .leading, spacing: 16) {
                    Text("Launchpad")
                        .font(.system(size: 20, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                        .padding(.horizontal, 40)
                    
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 20), count: 3), spacing: 20) {
                        LaunchpadTile(title: "Transfer Files", icon: "paperplane.fill", color: CRTheme.brandElectric) {
                            if let first = store.connectedDevices.first {
                                pendingFileTarget = first
                                showingFilePicker = true
                            }
                        }
                        LaunchpadTile(title: "Browse Device", icon: "internaldrive.fill", color: CRTheme.brandViolet) {
                            if store.connectedDevices.first != nil {
                                showingRemoteExplorer = true
                            }
                        }
                        LaunchpadTile(title: "Clipboard", icon: "doc.on.clipboard.fill", color: CRTheme.accentPink) {
                            withAnimation { store.selectedSection = .clipboard }
                        }
                        LaunchpadTile(title: "Speed Test", icon: "gauge.with.dots.needle.bottom.50percent", color: CRTheme.brandCyan) {
                            if let first = store.connectedDevices.first {
                                store.startSpeedTest(deviceId: first.id)
                                withAnimation { store.selectedSection = .transfers }
                            }
                        }
                        LaunchpadTile(title: "Remote Control", icon: "cursorarrow.rays", color: CRTheme.accentOrange) {
                            withAnimation { store.selectedSection = .remoteControl }
                        }
                        LaunchpadTile(title: "Settings", icon: "gearshape.fill", color: CRTheme.inkSubtle) {
                            withAnimation { store.selectedSection = .settings }
                        }
                    }
                    .padding(.horizontal, 40)
                }
                
                // 4. Recent Activity
                VStack(alignment: .leading, spacing: 16) {
                    Text("Recent Activity")
                        .font(.system(size: 20, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                        .padding(.horizontal, 40)
                    
                    VStack(alignment: .leading, spacing: 0) {
                        let recent = Array(store.activityFeed.filter { $0.isApplicable }.prefix(4))
                        if recent.isEmpty {
                            Text("No recent activity.")
                                .font(.system(size: 14))
                                .foregroundStyle(CRTheme.inkSoft)
                                .padding(16)
                        } else {
                            ForEach(Array(recent.enumerated()), id: \.offset) { index, entry in
                                ActivityRow(
                                    action: entry.summary,
                                    time: "Just now", // Since we don't have a relative time formatter readily available, we can just use the timestamp if needed, but for simplicity let's use the summary.
                                    icon: iconFor(kind: entry.kind)
                                )
                                if index < recent.count - 1 {
                                    Divider().padding(.horizontal, 16)
                                }
                            }
                        }
                    }
                    .background(CRTheme.surfaceStrong)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .padding(.horizontal, 40)
                }
                
                Spacer().frame(height: 60)
            }
        }
        .fileImporter(isPresented: $showingFilePicker, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            if case let .success(urls) = result {
                store.sendFiles(urls: urls, to: pendingFileTarget)
                pendingFileTarget = nil
            }
        }
        .sheet(isPresented: $showingRemoteExplorer) {
            if let first = store.connectedDevices.first {
                RemoteExplorerView(store: store, device: first)
            }
        }
    }
    
    private func iconFor(kind: String) -> String {
        switch kind {
        case "clipboard": return "doc.on.clipboard"
        case "file_transfer_started", "file_transfer_complete": return "paperplane.fill"
        case "app_installed": return "app.dashed"
        case "photo_synced": return "photo"
        default: return "bolt.fill"
        }
    }
}

// Subcomponents
struct LaunchpadTile: View {
    let title: String
    let icon: String
    let color: Color
    let action: () -> Void
    @State private var hovered = false
    
    var body: some View {
        Button(action: action) {
            VStack(spacing: 14) {
                ZStack {
                    Circle().fill(color.opacity(0.12)).frame(width: 54, height: 54)
                    Image(systemName: icon)
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundStyle(color)
                }
                Text(title)
                    .font(.system(size: 14, weight: .bold))
                    .foregroundStyle(CRTheme.ink)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 24)
            .background(CRTheme.surfaceStrong)
            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(CRTheme.stroke.opacity(0.5), lineWidth: 1))
            .shadow(color: Color.black.opacity(hovered ? 0.08 : 0.02), radius: hovered ? 12 : 4, y: hovered ? 6 : 2)
            .scaleEffect(hovered ? 1.02 : 1.0)
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .animation(.crSpring, value: hovered)
    }
}

struct SuggestionCard: View {
    let title: String
    let subtitle: String
    let icon: String
    let color: Color
    @State private var hovered = false
    
    var body: some View {
        HStack(spacing: 14) {
            Image(systemName: icon)
                .font(.system(size: 20))
                .foregroundStyle(color)
                .frame(width: 24)
            
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.system(size: 13, weight: .bold))
                    .foregroundStyle(CRTheme.ink)
                Text(subtitle)
                    .font(.system(size: 11.5))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            Spacer()
        }
        .padding(16)
        .frame(width: 260)
        .background(CRTheme.surfaceStrong)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(CRTheme.stroke.opacity(0.5), lineWidth: 1))
        .scaleEffect(hovered ? 1.02 : 1.0)
        .onHover { hovered = $0 }
        .animation(.crSpring, value: hovered)
    }
}

struct ActivityRow: View {
    let action: String
    let time: String
    let icon: String
    
    var body: some View {
        HStack(spacing: 16) {
            Image(systemName: icon)
                .font(.system(size: 16))
                .foregroundStyle(CRTheme.brandElectric)
            Text(action)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(CRTheme.ink)
            Spacer()
            Text(time)
                .font(.system(size: 12))
                .foregroundStyle(CRTheme.inkSoft)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 16)
    }
}

// MARK: - Right Column
struct LiveDevicePanel: View {
    @ObservedObject var store: DeskdropStore
    @State private var isPulsing = false
    
    private var device: ManagedDevice? {
        store.connectedDevices.first
    }
    
    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            if let dev = device {
                VStack(alignment: .leading, spacing: 28) {
                    
                    // Header Status
                    VStack(alignment: .leading, spacing: 8) {
                        Text(dev.name)
                            .font(.system(size: 24, weight: .bold, design: .rounded))
                            .foregroundStyle(CRTheme.ink)
                        
                        HStack(spacing: 6) {
                            Circle()
                                .fill(CRTheme.accentGreen)
                                .frame(width: 8, height: 8)
                                .scaleEffect(isPulsing ? 1.4 : 1.0)
                                .opacity(isPulsing ? 0.6 : 1.0)
                                .animation(.easeInOut(duration: 1).repeatForever(), value: isPulsing)
                            
                            Text("Connected")
                                .font(.system(size: 13, weight: .bold))
                                .foregroundStyle(CRTheme.accentGreen)
                            
                            Text("· Wi-Fi 6")
                                .font(.system(size: 13))
                                .foregroundStyle(CRTheme.inkSoft)
                            
                            Text("· \(dev.endpoint ?? "192.168.x.x")")
                                .font(.system(size: 13))
                                .foregroundStyle(CRTheme.inkSoft)
                        }
                    }
                    .onAppear { isPulsing = true }
                    
                    // Battery Row
                    if let bat = store.peerBatteries.first(where: { $0.deviceId == dev.id }) {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text("Battery")
                                    .font(.system(size: 13, weight: .semibold))
                                Spacer()
                                Text("\(bat.level)%")
                                    .font(.system(size: 13, weight: .bold))
                            }
                            GeometryReader { geo in
                                ZStack(alignment: .leading) {
                                    Capsule().fill(CRTheme.surfaceStrong)
                                    Capsule()
                                        .fill(bat.level < 20 ? CRTheme.accentRed : CRTheme.accentGreen)
                                        .frame(width: geo.size.width * (CGFloat(bat.level) / 100.0))
                                }
                            }
                            .frame(height: 8)
                        }
                    }

                    // Storage Row
                    if let storage = store.peerStorages.first(where: { $0.deviceId == dev.id }) {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Storage")
                                    .font(.system(size: 13, weight: .semibold))
                                Spacer()
                                let used = storage.totalBytes - storage.freeBytes
                                let usedGb = Double(used) / 1_000_000_000.0
                                let totalGb = Double(storage.totalBytes) / 1_000_000_000.0
                                Text("\(String(format: "%.1f", usedGb)) GB / \(String(format: "%.1f", totalGb)) GB")
                                    .font(.system(size: 12, weight: .medium))
                                    .foregroundStyle(CRTheme.inkSoft)
                            }
                            
                            GeometryReader { geo in
                                let used = max(0, storage.totalBytes - storage.freeBytes)
                                let total = max(1, storage.totalBytes)
                                let imgRatio = max(0, CGFloat(storage.imagesBytes) / CGFloat(total))
                                let vidRatio = max(0, CGFloat(storage.videosBytes) / CGFloat(total))
                                let otherRatio = max(0, CGFloat(used - storage.imagesBytes - storage.videosBytes) / CGFloat(total))
                                
                                ZStack(alignment: .leading) {
                                    // 1. Full-width background track
                                    Capsule()
                                        .fill(CRTheme.surfaceStrong)
                                        .frame(width: geo.size.width, height: 8)
                                    
                                    // 2. Filled segments
                                    HStack(spacing: 2) {
                                        if imgRatio > 0 {
                                            Rectangle().fill(Color.orange)
                                                .frame(width: max(0, geo.size.width * imgRatio - 1))
                                        }
                                        if vidRatio > 0 {
                                            Rectangle().fill(Color.purple)
                                                .frame(width: max(0, geo.size.width * vidRatio - 1))
                                        }
                                        if otherRatio > 0 {
                                            Rectangle().fill(Color.gray.opacity(0.6))
                                                .frame(width: max(0, geo.size.width * otherRatio - 1))
                                        }
                                    }
                                    .clipShape(Capsule())
                                    .frame(height: 8)
                                }
                            }
                            .frame(height: 8)
                            
                            HStack(spacing: 12) {
                                HStack(spacing: 4) {
                                    Circle().fill(Color.orange).frame(width: 6, height: 6)
                                    Text("Images")
                                }
                                HStack(spacing: 4) {
                                    Circle().fill(Color.purple).frame(width: 6, height: 6)
                                    Text("Videos")
                                }
                                HStack(spacing: 4) {
                                    Circle().fill(Color.gray.opacity(0.3)).frame(width: 6, height: 6)
                                    Text("Other")
                                }
                            }
                            .font(.system(size: 10))
                            .foregroundStyle(CRTheme.inkSubtle)
                        }
                    }

                    // Clipboard
                    if let lastClipboard = store.activityFeed.first(where: { $0.kind == "clipboard" }), let text = lastClipboard.text_preview {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Last Clipboard")
                                .font(.system(size: 11, weight: .bold))
                                .foregroundStyle(CRTheme.inkSubtle)
                            
                            Text(text)
                                .font(.system(size: 13, design: .monospaced))
                                .foregroundStyle(CRTheme.brandElectric)
                                .lineLimit(1)
                                .padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(CRTheme.brandElectric.opacity(0.1))
                                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                        }
                    }
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
                .padding(.top, 24)
            } else {
                // Not Connected State
                VStack(spacing: 16) {
                    Image(systemName: "antenna.radiowaves.left.and.right")
                        .font(.system(size: 40, weight: .ultraLight))
                        .foregroundStyle(CRTheme.inkSubtle)
                    Text("No active connection")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                    Text("Connect a device on your local network to view live insights here.")
                        .font(.system(size: 13))
                        .foregroundStyle(CRTheme.inkSoft)
                        .multilineTextAlignment(.center)
                }
                .padding(32)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }
}

struct LegendItem: View {
    let color: Color
    let text: String
    
    var body: some View {
        HStack(spacing: 4) {
            Circle().fill(color).frame(width: 6, height: 6)
            Text(text).font(.system(size: 11)).foregroundStyle(CRTheme.inkSoft)
        }
    }
}

struct MetricCard: View {
    let label: String
    let value: String
    let icon: String
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: icon)
                .foregroundStyle(CRTheme.inkSubtle)
                .font(.system(size: 14))
            
            VStack(alignment: .leading, spacing: 2) {
                Text(value)
                    .font(.system(size: 15, weight: .bold))
                    .foregroundStyle(CRTheme.ink)
                Text(label)
                    .font(.system(size: 11))
                    .foregroundStyle(CRTheme.inkSoft)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(CRTheme.surfaceStrong)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(CRTheme.stroke, lineWidth: 1))
    }
}


