// Deskdrop — Remote File Explorer Pro-Max (macOS)
// High-performance remote file browsing, date-grouped grids, batch multi-select, and instant pull/push over local Wi-Fi.

import SwiftUI
import AppKit

enum ExplorerViewMode: String, CaseIterable {
    case dateGrouped = "Date Grouped"
    case grid = "Grid"
    case list = "List"
    
    var icon: String {
        switch self {
        case .dateGrouped: return "calendar.day.timeline.left"
        case .grid: return "square.grid.2x2.fill"
        case .list: return "list.bullet"
        }
    }
}

struct FileDateGroup: Identifiable {
    let id: String
    let title: String
    let files: [IpcRemoteFileEntry]
}

struct RemoteExplorerView: View {
    @ObservedObject var store: DeskdropStore
    let device: ManagedDevice
    
    @State private var result: IpcRemoteFilesResult?
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var selectedCategory: String? = nil
    @State private var selectedSource: String? = nil
    @State private var searchQuery: String = ""
    @State private var viewMode: ExplorerViewMode = .dateGrouped
    @State private var showInspector: Bool = true
    @State private var isMultiSelect: Bool = false
    @State private var selectedFiles: Set<UInt64> = []
    @State private var selectedFile: IpcRemoteFileEntry? = nil
    
    @State private var thumbnailCache: [UInt64: NSImage] = [:]
    @State private var pullingFiles: Set<UInt64> = []
    @State private var pulledFiles: Set<UInt64> = []
    
    // Category tabs
    private let categories: [(id: String?, label: String, icon: String)] = [
        (nil, "All Files", "square.grid.2x2.fill"),
        ("Images", "Images", "photo.fill"),
        ("Videos", "Videos", "film.fill"),
        ("Audio", "Audio", "music.note.list"),
        ("Documents", "Documents", "doc.text.fill"),
        ("Apks", "APKs & Apps", "cube.box.fill"),
        ("Archives", "Archives", "archivebox.fill"),
        ("Other", "Other", "folder.fill")
    ]
    
    // Source tabs
    private let sources: [(id: String?, label: String, icon: String)] = [
        (nil, "All Sources", "tray.full.fill"),
        ("Camera", "Camera", "camera.fill"),
        ("WhatsApp", "WhatsApp", "message.fill"),
        ("Downloads", "Downloads", "arrow.down.circle.fill"),
        ("Other", "Other Folders", "folder.fill")
    ]
    
    var body: some View {
        VStack(spacing: 0) {
            // Top Header Bar
            headerView
            
            CRDivider()
            
            // Main 3-Column Layout
            HStack(spacing: 0) {
                // Left Column: Navigation & Filters (200px)
                sidebarFiltersView
                    .frame(width: 200)
                    .background(CRTheme.surfaceStrong)
                
                CRDivider()
                
                // Center Column: Main Expansive File Canvas (Flexible)
                ZStack(alignment: .bottom) {
                    VStack(spacing: 0) {
                        searchAndActionBar
                        CRDivider()
                        contentMainView
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(CRTheme.surface)
                    
                    // Floating Batch Action Bar when multiple items selected
                    if !selectedFiles.isEmpty {
                        floatingBatchActionBar
                            .padding(.bottom, 22)
                            .transition(.move(edge: .bottom).combined(with: .opacity))
                    }
                }
                
                // Right Column: Live Inspector Pane (300px toggleable)
                if showInspector {
                    CRDivider()
                    previewPaneView
                        .frame(width: 300)
                        .background(CRTheme.surfaceElevated)
                        .transition(.move(edge: .trailing))
                }
            }
        }
        .background(CRTheme.surfaceStrong)
        .frame(minWidth: 960, minHeight: 650)
        .onAppear {
            loadFiles()
        }
        .onChange(of: selectedCategory) { _ in loadFiles() }
        .onChange(of: selectedSource) { _ in loadFiles() }
    }
    
    // MARK: - Header Bar
    private var headerView: some View {
        HStack(spacing: 16) {
            ZStack {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(CRTheme.brandElectric.opacity(0.12))
                    .frame(width: 40, height: 40)
                Image(systemName: "internaldrive.fill")
                    .font(.system(size: 19, weight: .semibold))
                    .foregroundStyle(CRTheme.brandElectric)
            }
            
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 8) {
                    Text(device.name)
                        .font(.system(size: 16.5, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                    
                    HStack(spacing: 4) {
                        Circle()
                            .fill(CRTheme.accentGreen)
                            .frame(width: 6, height: 6)
                        Text("ONLINE • LOCAL WI-FI")
                            .font(.system(size: 9.5, weight: .bold))
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(CRTheme.accentGreen.opacity(0.16))
                    .foregroundStyle(CRTheme.accentGreen)
                    .clipShape(Capsule())
                }
                Text("Remote File Explorer • Instant Zero-Copy Access")
                    .font(.system(size: 11.5, weight: .medium))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            
            Spacer()
            
            if isLoading {
                ProgressView()
                    .controlSize(.small)
                    .padding(.trailing, 6)
            }
            
            // Import PC / Mac Files Button
            Button {
                importMacFiles()
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.up.doc.fill")
                    Text("Import Mac files...")
                }
                .font(.system(size: 12.5, weight: .semibold))
                .padding(.horizontal, 14)
                .padding(.vertical, 7)
            }
            .buttonStyle(PBPrimaryButtonStyle(tint: CRTheme.brandElectric))
            
            Button {
                loadFiles()
            } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
            }
            .buttonStyle(CRSecondaryButtonStyle())
            
            // Toggle Inspector Button
            Button {
                withAnimation(.spring(response: 0.28, dampingFraction: 0.82)) {
                    showInspector.toggle()
                }
            } label: {
                Image(systemName: "sidebar.right")
                    .font(.system(size: 14.5, weight: .semibold))
                    .foregroundStyle(showInspector ? CRTheme.brandElectric : CRTheme.inkSoft)
                    .frame(width: 32, height: 32)
                    .background(showInspector ? CRTheme.brandElectric.opacity(0.14) : CRTheme.surfaceStrong)
                    .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .strokeBorder(showInspector ? CRTheme.brandElectric.opacity(0.4) : CRTheme.stroke, lineWidth: 0.5)
                    }
            }
            .buttonStyle(.plain)
            .help("Toggle Live Inspector Panel")
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 14)
        .background(CRTheme.surfaceElevated)
    }
    
    // MARK: - Sidebar Filters (Left Column)
    private var sidebarFiltersView: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 22) {
                // Categories
                VStack(alignment: .leading, spacing: 6) {
                    Text("CATEGORIES")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(CRTheme.inkSubtle)
                        .padding(.horizontal, 14)
                    
                    ForEach(categories, id: \.label) { cat in
                        filterRow(
                            label: cat.label,
                            icon: cat.icon,
                            isSelected: selectedCategory == cat.id,
                            count: countForCategory(cat.id)
                        ) {
                            selectedCategory = cat.id
                        }
                    }
                }
                
                // Sources
                VStack(alignment: .leading, spacing: 6) {
                    Text("SOURCES")
                        .font(.system(size: 10, weight: .bold))
                        .foregroundStyle(CRTheme.inkSubtle)
                        .padding(.horizontal, 14)
                    
                    ForEach(sources, id: \.label) { src in
                        filterRow(
                            label: src.label,
                            icon: src.icon,
                            isSelected: selectedSource == src.id,
                            count: countForSource(src.id)
                        ) {
                            selectedSource = src.id
                        }
                    }
                }
            }
            .padding(.vertical, 16)
        }
    }
    
    private func filterRow(label: String, icon: String, isSelected: Bool, count: UInt32?, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: icon)
                    .font(.system(size: 14))
                    .frame(width: 20)
                
                Text(label)
                    .font(.system(size: 12.5, weight: isSelected ? .bold : .medium))
                    .lineLimit(1)
                
                Spacer()
                
                if let count = count {
                    Text("\(count)")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(isSelected ? Color.white.opacity(0.9) : CRTheme.inkSubtle)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 2.5)
                        .background(isSelected ? Color.white.opacity(0.22) : CRTheme.surfaceElevated)
                        .clipShape(Capsule())
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8.5)
            .background(isSelected ? CRTheme.brandElectric : Color.clear)
            .foregroundStyle(isSelected ? Color.white : CRTheme.inkSoft)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .padding(.horizontal, 8)
        }
        .buttonStyle(.plain)
    }
    
    // MARK: - Search and Action Bar (Top of Center Canvas)
    private var searchAndActionBar: some View {
        HStack(alignment: .center, spacing: 14) {
            // Breadcrumb / Title
            HStack(spacing: 6) {
                Text(selectedCategory != nil ? selectedCategory! : (selectedSource != nil ? selectedSource! : "All Files"))
                    .font(.system(size: 15, weight: .bold))
                    .foregroundStyle(CRTheme.ink)
                if let res = result {
                    Text("(\(res.total_matching))")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(CRTheme.inkSoft)
                }
            }
            
            Spacer()
            
            // Right-side unified control bar
            HStack(alignment: .center, spacing: 10) {
                // Search Input
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .foregroundStyle(CRTheme.inkSubtle)
                        .font(.system(size: 12))
                    TextField("Search files by name...", text: $searchQuery)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12.5))
                        .frame(width: 170)
                        .onSubmit { loadFiles() }
                    if !searchQuery.isEmpty {
                        Button {
                            searchQuery = ""
                            loadFiles()
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundStyle(CRTheme.inkSubtle)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .frame(height: 32)
                .padding(.horizontal, 10)
                .background(CRTheme.surfaceStrong)
                .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .strokeBorder(CRTheme.stroke, lineWidth: 0.5)
                }
                
                // Multi-Select Checkbox Toggle
                Button {
                    withAnimation(.easeInOut(duration: 0.18)) {
                        isMultiSelect.toggle()
                        if !isMultiSelect {
                            selectedFiles.removeAll()
                        }
                    }
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: isMultiSelect ? "checkmark.circle.fill" : "checkmark.circle")
                        Text("Select")
                    }
                    .font(.system(size: 12, weight: isMultiSelect ? .bold : .medium))
                    .frame(height: 32)
                    .padding(.horizontal, 11)
                    .background(isMultiSelect ? CRTheme.brandElectric.opacity(0.16) : CRTheme.surfaceStrong)
                    .foregroundStyle(isMultiSelect ? CRTheme.brandElectric : CRTheme.inkSoft)
                    .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .strokeBorder(isMultiSelect ? CRTheme.brandElectric : CRTheme.stroke, lineWidth: 0.5)
                    }
                }
                .buttonStyle(.plain)
                
                // View Mode Picker
                HStack(spacing: 1) {
                    ForEach(ExplorerViewMode.allCases, id: \.self) { mode in
                        Button {
                            withAnimation(.easeInOut(duration: 0.15)) {
                                viewMode = mode
                            }
                        } label: {
                            Image(systemName: mode.icon)
                                .font(.system(size: 13, weight: viewMode == mode ? .semibold : .regular))
                                .foregroundStyle(viewMode == mode ? CRTheme.brandElectric : CRTheme.inkSubtle)
                                .frame(width: 32, height: 28)
                                .background(viewMode == mode ? CRTheme.surfaceElevated : Color.clear)
                                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                        }
                        .buttonStyle(.plain)
                        .help(mode.rawValue)
                    }
                }
                .frame(height: 32)
                .padding(2)
                .background(CRTheme.surfaceStrong)
                .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 7, style: .continuous)
                        .strokeBorder(CRTheme.stroke, lineWidth: 0.5)
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
    }
    
    // MARK: - Main Content Area (Center Canvas)
    private var contentMainView: some View {
        ZStack {
            if let err = errorMessage {
                VStack(spacing: 14) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 36))
                        .foregroundStyle(CRTheme.accentOrange)
                    Text("Error accessing remote files")
                        .font(.system(size: 15, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                    Text(err)
                        .font(.system(size: 12.5))
                        .foregroundStyle(CRTheme.inkSoft)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 30)
                    Button("Retry") { loadFiles() }
                        .buttonStyle(CRPrimaryButtonStyle())
                }
                .padding(40)
            } else if let res = result, res.files.isEmpty && !isLoading {
                VStack(spacing: 14) {
                    Image(systemName: "folder.badge.questionmark")
                        .font(.system(size: 40))
                        .foregroundStyle(CRTheme.inkSubtle)
                    Text("No files found")
                        .font(.system(size: 15, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                    Text("Try selecting a different category or clearing search.")
                        .font(.system(size: 12.5))
                        .foregroundStyle(CRTheme.inkSoft)
                }
            } else if let res = result {
                ScrollView(.vertical, showsIndicators: true) {
                    VStack(alignment: .leading, spacing: 22) {
                        switch viewMode {
                        case .dateGrouped:
                            dateGroupedView(for: res.files)
                        case .grid:
                            LazyVGrid(columns: [GridItem(.adaptive(minimum: 145, maximum: 180), spacing: 16)], alignment: .leading, spacing: 16) {
                                ForEach(res.files) { file in
                                    fileGridCard(for: file)
                                }
                            }
                            .padding(20)
                        case .list:
                            LazyVStack(alignment: .leading, spacing: 5) {
                                ForEach(res.files) { file in
                                    fileListRow(for: file)
                                }
                            }
                            .padding(16)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.bottom, selectedFiles.isEmpty ? 24 : 95)
                }
            } else if isLoading {
                ProgressView("Connecting to \(device.name)...")
                    .controlSize(.regular)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
    
    // MARK: - Date Grouped View
    private func dateGroupedView(for files: [IpcRemoteFileEntry]) -> some View {
        VStack(alignment: .leading, spacing: 26) {
            ForEach(groupedFiles(from: files)) { group in
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 8) {
                        Image(systemName: "calendar")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(CRTheme.brandElectric)
                        Text(group.title)
                            .font(.system(size: 14, weight: .bold))
                            .foregroundStyle(CRTheme.ink)
                        Spacer()
                        Text("\(group.files.count) items")
                            .font(.system(size: 11.5, weight: .medium))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                    .padding(.horizontal, 2)
                    
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 145, maximum: 180), spacing: 16)], alignment: .leading, spacing: 16) {
                        ForEach(group.files) { file in
                            fileGridCard(for: file)
                        }
                    }
                }
            }
        }
        .padding(20)
    }
    
    // MARK: - Grid Card (Clean 1:1 Square Box + Two-Line Title)
    private func fileGridCard(for file: IpcRemoteFileEntry) -> some View {
        Button {
            handleFileSelection(file)
        } label: {
            ZStack(alignment: .topTrailing) {
                VStack(alignment: .leading, spacing: 9) {
                    // 1:1 Square Thumbnail / Icon Tile
                    ZStack {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(backgroundForTile(file.category))
                            .aspectRatio(1.0, contentMode: .fit)
                        
                        if let thumb = thumbnailCache[file.file_id] {
                            Image(nsImage: thumb)
                                .resizable()
                                .scaledToFill()
                                .frame(minWidth: 0, maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
                                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                        } else {
                            VStack(spacing: 6) {
                                Image(systemName: iconForMime(file.mime_type, cat: file.category))
                                    .font(.system(size: 34, weight: .medium))
                                    .foregroundStyle(colorForCategory(file.category))
                                if file.category.lowercased() == "audio" || file.mime_type.contains("audio") {
                                    Text("AUDIO")
                                        .font(.system(size: 9, weight: .bold))
                                        .foregroundStyle(colorForCategory(file.category).opacity(0.8))
                                }
                            }
                        }
                    }
                    .onAppear { fetchThumbnailIfNeeded(for: file) }
                    
                    VStack(alignment: .leading, spacing: 3) {
                        Text(file.display_name)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(CRTheme.ink)
                            .lineLimit(2)
                            .multilineTextAlignment(.leading)
                        
                        HStack {
                            Text(formatSize(file.size_bytes))
                            Spacer()
                            if pullingFiles.contains(file.file_id) {
                                ProgressView().controlSize(.small)
                            } else if pulledFiles.contains(file.file_id) {
                                Image(systemName: "checkmark.circle.fill")
                                    .foregroundStyle(CRTheme.brandElectric)
                                    .font(.system(size: 12))
                            }
                        }
                        .font(.system(size: 10.5))
                        .foregroundStyle(CRTheme.inkSoft)
                    }
                }
                .padding(11)
                .background(isSelected(file) ? CRTheme.brandElectric.opacity(0.12) : CRTheme.surfaceElevated)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .strokeBorder(isSelected(file) ? CRTheme.brandElectric : CRTheme.stroke, lineWidth: isSelected(file) ? 1.5 : 0.5)
                }
                
                // Checkbox Indicator in Multi-Select Mode
                if isMultiSelect {
                    Image(systemName: selectedFiles.contains(file.file_id) ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(selectedFiles.contains(file.file_id) ? CRTheme.brandElectric : Color.white.opacity(0.85))
                        .background(Circle().fill(selectedFiles.contains(file.file_id) ? Color.white : Color.black.opacity(0.45)).frame(width: 17, height: 17))
                        .padding(9)
                }
            }
        }
        .buttonStyle(.plain)
    }
    
    // MARK: - List Row
    private func fileListRow(for file: IpcRemoteFileEntry) -> some View {
        Button {
            handleFileSelection(file)
        } label: {
            HStack(spacing: 12) {
                if isMultiSelect {
                    Image(systemName: selectedFiles.contains(file.file_id) ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(selectedFiles.contains(file.file_id) ? CRTheme.brandElectric : CRTheme.inkSubtle)
                }
                
                ZStack {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(backgroundForTile(file.category))
                        .frame(width: 40, height: 40)
                    
                    if let thumb = thumbnailCache[file.file_id] {
                        Image(nsImage: thumb)
                            .resizable()
                            .scaledToFill()
                            .frame(width: 40, height: 40)
                            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    } else {
                        Image(systemName: iconForMime(file.mime_type, cat: file.category))
                            .font(.system(size: 18))
                            .foregroundStyle(colorForCategory(file.category))
                    }
                }
                .onAppear { fetchThumbnailIfNeeded(for: file) }
                
                VStack(alignment: .leading, spacing: 3) {
                    Text(file.display_name)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(CRTheme.ink)
                        .lineLimit(1)
                    HStack(spacing: 8) {
                        Text(formatSize(file.size_bytes))
                        Text("•")
                        Text(formatDate(file.date_modified))
                        if !file.source.isEmpty {
                            Text("•")
                            Text(file.source.capitalized)
                        }
                    }
                    .font(.system(size: 11))
                    .foregroundStyle(CRTheme.inkSoft)
                }
                
                Spacer()
                
                if pullingFiles.contains(file.file_id) {
                    ProgressView().controlSize(.small)
                } else if pulledFiles.contains(file.file_id) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(CRTheme.brandElectric)
                        .font(.system(size: 15))
                } else {
                    Image(systemName: "arrow.down.to.line.alt")
                        .font(.system(size: 13))
                        .foregroundStyle(CRTheme.inkSubtle)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(isSelected(file) ? CRTheme.brandElectric.opacity(0.12) : CRTheme.surfaceElevated)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(isSelected(file) ? CRTheme.brandElectric : Color.clear, lineWidth: 1)
            }
        }
        .buttonStyle(.plain)
    }
    
    // MARK: - Floating Batch Action Bar (Bottom of Center Canvas)
    private var floatingBatchActionBar: some View {
        HStack(spacing: 18) {
            HStack(spacing: 8) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(CRTheme.brandElectric)
                Text("\(selectedFiles.count) items selected")
                    .font(.system(size: 13.5, weight: .bold))
                    .foregroundStyle(CRTheme.ink)
                Text("(\(formatSize(totalSelectedBytes())))")
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            
            Spacer()
            
            Button {
                pullSelectedBatch()
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.down.to.line.alt")
                    Text("Pull \(selectedFiles.count) Files to Mac")
                }
                .font(.system(size: 13, weight: .semibold))
                .padding(.horizontal, 18)
                .padding(.vertical, 8.5)
            }
            .buttonStyle(PBPrimaryButtonStyle(tint: CRTheme.brandElectric))
            
            Button {
                withAnimation {
                    selectedFiles.removeAll()
                }
            } label: {
                Text("Clear")
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 22)
        .padding(.vertical, 14)
        .background(CRTheme.surfaceElevated.opacity(0.97))
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(CRTheme.brandElectric, lineWidth: 1.2)
        }
        .shadow(color: Color.black.opacity(0.28), radius: 14, x: 0, y: 7)
        .frame(maxWidth: 600)
    }
    
    // MARK: - Right Live Inspector Pane (300px)
    private var previewPaneView: some View {
        VStack(spacing: 0) {
            // Header bar
            HStack(spacing: 8) {
                Image(systemName: "eye.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(CRTheme.brandElectric)
                Text("LIVE INSPECTOR")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(CRTheme.inkSubtle)
                Spacer()
                if let file = selectedFile {
                    Text(file.category.capitalized)
                        .font(.system(size: 10, weight: .bold))
                        .padding(.horizontal, 9)
                        .padding(.vertical, 3.5)
                        .background(colorForCategory(file.category).opacity(0.18))
                        .foregroundStyle(colorForCategory(file.category))
                        .clipShape(Capsule())
                }
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 14)
            .background(CRTheme.surfaceElevated)
            
            CRDivider()
            
            if let file = selectedFile {
                ScrollView(.vertical, showsIndicators: true) {
                    VStack(alignment: .leading, spacing: 22) {
                        // Visual Preview Box
                        ZStack {
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .fill(backgroundForTile(file.category))
                                .frame(minHeight: 230, maxHeight: 330)
                                .overlay {
                                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                                        .strokeBorder(CRTheme.stroke, lineWidth: 0.5)
                                }
                            
                            if let thumb = thumbnailCache[file.file_id] {
                                Image(nsImage: thumb)
                                    .resizable()
                                    .scaledToFit()
                                    .frame(minHeight: 230, maxHeight: 320)
                                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                                    .padding(8)
                            } else {
                                VStack(spacing: 14) {
                                    Image(systemName: iconForMime(file.mime_type, cat: file.category))
                                        .font(.system(size: 58, weight: .medium))
                                        .foregroundStyle(colorForCategory(file.category))
                                    Text(file.display_name)
                                        .font(.system(size: 13.5, weight: .semibold))
                                        .foregroundStyle(CRTheme.ink)
                                        .multilineTextAlignment(.center)
                                        .padding(.horizontal, 20)
                                }
                            }
                        }
                        .onAppear { fetchThumbnailIfNeeded(for: file) }
                        .onChange(of: file.file_id) { _ in fetchThumbnailIfNeeded(for: file) }
                        
                        // Action Toolbar
                        VStack(spacing: 11) {
                            Button {
                                pullFile(file)
                            } label: {
                                HStack(spacing: 8) {
                                    if pullingFiles.contains(file.file_id) {
                                        ProgressView().controlSize(.small)
                                        Text("Pulling to Mac...")
                                    } else if pulledFiles.contains(file.file_id) {
                                        Image(systemName: "checkmark.circle.fill")
                                        Text("Transferred to Mac")
                                    } else {
                                        Image(systemName: "arrow.down.to.line.alt")
                                        Text("Pull to Mac")
                                    }
                                }
                                .font(.system(size: 13.5, weight: .semibold))
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 9.5)
                            }
                            .buttonStyle(PBPrimaryButtonStyle(tint: pulledFiles.contains(file.file_id) ? CRTheme.inkSoft : CRTheme.brandElectric))
                            .disabled(pullingFiles.contains(file.file_id))
                            
                            if pulledFiles.contains(file.file_id) {
                                HStack(spacing: 10) {
                                    Button {
                                        openPulledFile(file)
                                    } label: {
                                        Label("Open File", systemImage: "arrow.up.right.square")
                                            .font(.system(size: 12, weight: .semibold))
                                            .frame(maxWidth: .infinity)
                                            .padding(.vertical, 7)
                                    }
                                    .buttonStyle(CRSecondaryButtonStyle())
                                    
                                    Button {
                                        revealInFinder(file)
                                    } label: {
                                        Label("Reveal in Finder", systemImage: "folder")
                                            .font(.system(size: 12, weight: .semibold))
                                            .frame(maxWidth: .infinity)
                                            .padding(.vertical, 7)
                                    }
                                    .buttonStyle(CRSecondaryButtonStyle())
                                }
                            }
                        }
                        
                        // Metadata Card
                        VStack(alignment: .leading, spacing: 14) {
                            Text("FILE DETAILS")
                                .font(.system(size: 10, weight: .bold))
                                .foregroundStyle(CRTheme.inkSubtle)
                            
                            VStack(alignment: .leading, spacing: 10) {
                                metadataRow(label: "Name", value: file.display_name)
                                CRDivider()
                                metadataRow(label: "Size", value: formatSize(file.size_bytes) + " (\(file.size_bytes) bytes)")
                                CRDivider()
                                metadataRow(label: "Modified", value: formatDate(file.date_modified))
                                CRDivider()
                                metadataRow(label: "MIME Type", value: file.mime_type)
                                CRDivider()
                                metadataRow(label: "Source", value: file.source.isEmpty ? "Local Device" : file.source.capitalized)
                                CRDivider()
                                metadataRow(label: "Path", value: file.content_uri)
                            }
                            .padding(16)
                            .background(CRTheme.surfaceStrong)
                            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                            .overlay {
                                RoundedRectangle(cornerRadius: 12, style: .continuous)
                                    .strokeBorder(CRTheme.stroke, lineWidth: 0.5)
                            }
                        }
                    }
                    .padding(18)
                }
            } else {
                // Empty selection state
                VStack(spacing: 16) {
                    Image(systemName: "photo.on.rectangle.angled")
                        .font(.system(size: 44))
                        .foregroundStyle(CRTheme.inkSubtle)
                    Text("Live Inspector")
                        .font(.system(size: 15, weight: .bold))
                        .foregroundStyle(CRTheme.ink)
                    Text("Select any item from the grid on the left to inspect high-res previews and metadata.")
                        .font(.system(size: 12.5))
                        .foregroundStyle(CRTheme.inkSoft)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(CRTheme.surfaceElevated)
    }
    
    private func metadataRow(label: String, value: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Text(label)
                .font(.system(size: 11.5, weight: .semibold))
                .foregroundStyle(CRTheme.inkSoft)
                .frame(width: 75, alignment: .leading)
            Text(value)
                .font(.system(size: 11.5, weight: .regular))
                .foregroundStyle(CRTheme.ink)
                .textSelection(.enabled)
                .lineLimit(3)
        }
    }
    
    // MARK: - Helpers & Actions
    
    private func isSelected(_ file: IpcRemoteFileEntry) -> Bool {
        if isMultiSelect {
            return selectedFiles.contains(file.file_id)
        }
        return selectedFile?.file_id == file.file_id
    }
    
    private func handleFileSelection(_ file: IpcRemoteFileEntry) {
        if isMultiSelect {
            if selectedFiles.contains(file.file_id) {
                selectedFiles.remove(file.file_id)
            } else {
                selectedFiles.insert(file.file_id)
            }
        } else {
            selectedFile = file
        }
    }
    
    private func totalSelectedBytes() -> UInt64 {
        guard let files = result?.files else { return 0 }
        return files.filter { selectedFiles.contains($0.file_id) }.reduce(0) { $0 + $1.size_bytes }
    }
    
    private func pullSelectedBatch() {
        guard let files = result?.files else { return }
        let toPull = files.filter { selectedFiles.contains($0.file_id) }
        for file in toPull {
            pullFile(file)
        }
    }
    
    private func importMacFiles() {
        let panel = NSOpenPanel()
        panel.title = "Select Files to Import to \(device.name)"
        panel.allowsMultipleSelection = true
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        if panel.runModal() == .OK {
            store.sendFiles(urls: panel.urls, to: device)
        }
    }
    
    private func groupedFiles(from files: [IpcRemoteFileEntry]) -> [FileDateGroup] {
        let calendar = Calendar.current
        var dict: [String: [IpcRemoteFileEntry]] = [:]
        var order: [String] = []
        
        for file in files {
            let date = Date(timeIntervalSince1970: TimeInterval(file.date_modified))
            let key: String
            if calendar.isDateInToday(date) {
                key = "Today"
            } else if calendar.isDateInYesterday(date) {
                key = "Yesterday"
            } else {
                let df = DateFormatter()
                df.dateStyle = .medium
                key = df.string(from: date)
            }
            
            if dict[key] == nil {
                order.append(key)
            }
            dict[key, default: []].append(file)
        }
        
        return order.map { key in
            FileDateGroup(id: key, title: key, files: dict[key] ?? [])
        }
    }
    
    private func loadFiles() {
        isLoading = true
        errorMessage = nil
        let target = device.id
        let cat = selectedCategory
        let src = selectedSource
        let query = searchQuery.isEmpty ? nil : searchQuery
        
        Task {
            do {
                let res = try await store.queryRemoteFiles(
                    targetDevice: target,
                    summaryOnly: false,
                    category: cat,
                    source: src,
                    searchQuery: query,
                    offset: 0,
                    limit: 100
                )
                _ = await MainActor.run {
                    self.result = res
                    self.isLoading = false
                    if let err = res.error, !err.isEmpty {
                        self.errorMessage = err
                    }
                    if self.selectedFile == nil || !res.files.contains(where: { $0.file_id == self.selectedFile?.file_id }) {
                        self.selectedFile = res.files.first
                    }
                }
            } catch {
                _ = await MainActor.run {
                    self.isLoading = false
                    self.errorMessage = error.localizedDescription
                }
            }
        }
    }
    
    private func fetchThumbnailIfNeeded(for file: IpcRemoteFileEntry) {
        let cat = file.category.lowercased()
        guard cat.hasPrefix("image") || cat.hasPrefix("video") else { return }
        guard thumbnailCache[file.file_id] == nil else { return }
        let target = device.id
        let id = file.file_id
        
        Task {
            if let data = try? await store.requestRemoteThumbnail(targetDevice: target, fileId: id, sizePx: 256),
               let img = NSImage(data: data) {
                _ = await MainActor.run {
                    self.thumbnailCache[id] = img
                }
            }
        }
    }
    
    private func pullFile(_ file: IpcRemoteFileEntry) {
        pullingFiles.insert(file.file_id)
        let target = device.id
        let id = file.file_id
        
        Task {
            do {
                try await store.pullRemoteFile(targetDevice: target, fileId: id)
                _ = await MainActor.run {
                    pullingFiles.remove(id)
                    pulledFiles.insert(id)
                }
            } catch {
                _ = await MainActor.run {
                    pullingFiles.remove(id)
                }
            }
        }
    }
    
    private func openPulledFile(_ file: IpcRemoteFileEntry) {
        let downloads = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        if let downloads = downloads {
            let fileUrl = downloads.appendingPathComponent(file.display_name)
            if FileManager.default.fileExists(atPath: fileUrl.path) {
                NSWorkspace.shared.open(fileUrl)
                return
            }
        }
    }
    
    private func revealInFinder(_ file: IpcRemoteFileEntry) {
        let downloads = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        if let downloads = downloads {
            let fileUrl = downloads.appendingPathComponent(file.display_name)
            if FileManager.default.fileExists(atPath: fileUrl.path) {
                NSWorkspace.shared.activateFileViewerSelecting([fileUrl])
                return
            }
        }
        if let downloads = downloads {
            NSWorkspace.shared.open(downloads)
        }
    }
    
    private func countForCategory(_ cat: String?) -> UInt32? {
        guard let summary = result?.summary else { return nil }
        guard let cat = cat?.lowercased() else { return result?.total_matching }
        switch cat {
        case "image", "images": return summary.type_counts.images
        case "video", "videos": return summary.type_counts.videos
        case "audio": return summary.type_counts.audio
        case "document", "documents": return summary.type_counts.documents
        case "apk", "apks": return summary.type_counts.apks
        case "archive", "archives": return summary.type_counts.archives
        default: return nil
        }
    }
    
    private func countForSource(_ src: String?) -> UInt32? {
        guard let summary = result?.summary else { return nil }
        guard let src = src?.lowercased() else { return nil }
        switch src {
        case "camera": return summary.source_counts.camera
        case "whatsapp": return summary.source_counts.whatsapp
        case "downloads": return summary.source_counts.downloads
        default: return nil
        }
    }
    
    private func formatSize(_ bytes: UInt64) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useAll]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
    
    private func formatDate(_ epochSec: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(epochSec))
        let df = DateFormatter()
        df.dateStyle = .short
        return df.string(from: date)
    }
    
    private func iconForMime(_ mime: String, cat: String) -> String {
        switch cat.lowercased() {
        case "image", "images": return "photo.fill"
        case "video", "videos": return "film.fill"
        case "audio": return "music.note.list"
        case "apk", "apks": return "cube.box.fill"
        case "archive", "archives": return "doc.zipper"
        default:
            if mime.contains("pdf") { return "doc.richtext.fill" }
            return "doc.fill"
        }
    }
    
    private func colorForCategory(_ cat: String) -> Color {
        switch cat.lowercased() {
        case "image", "images": return CRTheme.brandElectric
        case "video", "videos": return CRTheme.accentPurple
        case "audio": return CRTheme.accentPink
        case "apk", "apks": return CRTheme.accentGreen
        case "archive", "archives": return CRTheme.accentOrange
        default: return CRTheme.inkSoft
        }
    }
    
    private func backgroundForTile(_ cat: String) -> Color {
        switch cat.lowercased() {
        case "image", "images": return CRTheme.brandElectric.opacity(0.10)
        case "video", "videos": return CRTheme.accentPurple.opacity(0.10)
        case "audio": return CRTheme.accentPink.opacity(0.12)
        case "apk", "apks": return CRTheme.accentGreen.opacity(0.10)
        case "archive", "archives": return CRTheme.accentOrange.opacity(0.10)
        default: return CRTheme.surfaceStrong
        }
    }
}
