import SwiftUI

struct TransfersDashboardView: View {
    @ObservedObject var store: DeskdropStore
    @Environment(\.colorScheme) var colorScheme
    
    var historyItems: [IpcActivityEntry] {
        store.activityFeed.filter { $0.kind == "file" }
    }
    
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 32) {
                
                // MARK: - Header
                VStack(alignment: .leading, spacing: 4) {
                    Text("Transfers")
                        .font(.system(size: 28, weight: .semibold, design: .rounded))
                        .foregroundStyle(CRTheme.ink)
                    Text("Manage active files and view recent history")
                        .font(.system(size: 14, weight: .regular))
                        .foregroundStyle(CRTheme.inkSoft)
                }
                .padding(.top, 40)
                .padding(.horizontal, 40)
                
                // MARK: - Active Transfers
                VStack(alignment: .leading, spacing: 16) {
                    Text("ACTIVE")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(CRTheme.inkSoft.opacity(0.6))
                        .kerning(1.2)
                        .padding(.horizontal, 40)
                    
                    if store.activeTransfers.isEmpty {
                        Text("No active transfers")
                            .font(.system(size: 13, weight: .regular))
                            .foregroundStyle(CRTheme.inkSoft.opacity(0.8))
                            .padding(.horizontal, 40)
                            .padding(.bottom, 8)
                    } else {
                        VStack(spacing: 12) {
                            ForEach(store.activeTransfers) { transfer in
                                ActiveTransferCard(transfer: transfer, store: store)
                            }
                        }
                        .padding(.horizontal, 40)
                    }
                }
                
                // MARK: - Recent History
                VStack(alignment: .leading, spacing: 16) {
                    Text("RECENT HISTORY")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(CRTheme.inkSoft.opacity(0.6))
                        .kerning(1.2)
                        .padding(.horizontal, 40)
                    
                    if historyItems.isEmpty {
                        Text("No recent file transfers")
                            .font(.system(size: 13, weight: .regular))
                            .foregroundStyle(CRTheme.inkSoft.opacity(0.8))
                            .padding(.horizontal, 40)
                            .padding(.bottom, 40)
                    } else {
                        VStack(spacing: 0) {
                            ForEach(historyItems) { item in
                                TransferHistoryRow(entry: item)
                                if item.id != historyItems.last?.id {
                                    Divider()
                                        .opacity(0.5)
                                        .padding(.leading, 48) // Align with text content
                                }
                            }
                        }
                        .padding(.horizontal, 40)
                        .padding(.bottom, 40)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color.clear)
    }
}

// MARK: - Active Transfer Card

private struct ActiveTransferCard: View {
    let transfer: FileTransferState
    @ObservedObject var store: DeskdropStore
    
    var progressColor: Color {
        switch transfer.status {
        case .paused: return CRTheme.stroke
        case .failed: return CRTheme.accentRed
        default: return CRTheme.brandElectric
        }
    }
    
    var statusText: String {
        switch transfer.status {
        case .incoming: return "Waiting for approval..."
        case .transferring:
            var text = "\(transfer.percent)% • \(transfer.formattedSize)"
            if let speed = transfer.speedBps {
                let mbps = Double(speed) / 1_048_576.0
                if mbps >= 1.0 { text += String(format: " • %.1f MB/s", mbps) }
                else { text += String(format: " • %.0f KB/s", Double(speed) / 1024.0) }
            }
            if let eta = transfer.etaSecs, eta > 0 {
                text += " • \(eta)s remaining"
            }
            return text
        case .paused: return "Paused • \(transfer.percent)% of \(transfer.formattedSize)"
        case .verifying: return "Verifying..."
        case .complete: return "Complete"
        case .failed(let r): return "Failed: \(r)"
        case .cancelled: return "Cancelled"
        }
    }
    
    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            // Icon
            ZStack {
                Circle()
                    .fill(CRTheme.surfaceStrong)
                    .frame(width: 40, height: 40)
                Image(systemName: "doc.fill")
                    .font(.system(size: 16))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            
            // Details
            VStack(alignment: .leading, spacing: 4) {
                Text(transfer.fileName)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(CRTheme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
                
                Text(statusText)
                    .font(.system(size: 12, weight: .regular))
                    .foregroundStyle(CRTheme.inkSoft)
                
                // Progress Bar
                if case .incoming = transfer.status {
                    // No progress bar for incoming
                } else if case .failed = transfer.status {
                    // No progress bar for failed
                } else {
                    ProgressView(value: Double(transfer.percent), total: 100.0)
                        .tint(progressColor)
                        .frame(height: 4)
                        .padding(.top, 4)
                }
            }
            
            Spacer(minLength: 16)
            
            // Actions
            HStack(spacing: 8) {
                if case .incoming = transfer.status {
                    actionButton(icon: "checkmark", color: .green) { store.acceptFileTransfer(transfer) }
                    actionButton(icon: "xmark", color: .red) { store.rejectFileTransfer(transfer) }
                } else if case .transferring = transfer.status {
                    actionButton(icon: "pause.fill") { store.pauseFileTransfer(transfer) }
                    actionButton(icon: "xmark") { store.cancelFileTransfer(transfer) }
                } else if case .paused = transfer.status {
                    actionButton(icon: "play.fill") { store.resumeFileTransfer(transfer) }
                    actionButton(icon: "xmark") { store.cancelFileTransfer(transfer) }
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
        .crCard(cornerRadius: 12)
    }
    
    private func actionButton(icon: String, color: Color = CRTheme.inkSoft, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(color)
                .frame(width: 28, height: 28)
                .background(Color.black.opacity(0.04))
                .clipShape(Circle())
        }
        .buttonStyle(.plain)
        .crHoverScale(scale: 1.05)
    }
}

// MARK: - History Row

private struct TransferHistoryRow: View {
    let entry: IpcActivityEntry
    
    var formattedTime: String {
        let date = Date(timeIntervalSince1970: TimeInterval(entry.timestamp_ms) / 1000.0)
        let formatter = DateFormatter()
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
    
    var displaySize: String {
        guard let b = entry.file_bytes else { return "Unknown size" }
        let mb = Double(b) / 1_048_576.0
        if mb >= 1.0 { return String(format: "%.1f MB", mb) }
        let kb = Double(b) / 1_024.0
        if kb >= 1.0 { return String(format: "%.0f KB", kb) }
        return "\(b) B"
    }
    
    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            // Icon
            Image(systemName: "arrow.down.doc")
                .font(.system(size: 18, weight: .light))
                .foregroundStyle(CRTheme.inkSoft.opacity(0.8))
                .frame(width: 32)
            
            // Details
            VStack(alignment: .leading, spacing: 2) {
                Text(entry.file_name ?? "Unknown File")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(CRTheme.ink)
                    .lineLimit(1)
                    .truncationMode(.middle)
                
                Text("Received from \(entry.device_name) • \(displaySize)")
                    .font(.system(size: 12, weight: .regular))
                    .foregroundStyle(CRTheme.inkSoft)
            }
            
            Spacer(minLength: 16)
            
            // Time
            Text(formattedTime)
                .font(.system(size: 12, weight: .regular))
                .foregroundStyle(CRTheme.inkSoft.opacity(0.6))
            
            // Folder action
            if let dest = entry.dest_path, !dest.isEmpty {
                Button {
                    let url = URL(fileURLWithPath: dest)
                    NSWorkspace.shared.activateFileViewerSelecting([url])
                } label: {
                    Image(systemName: "folder")
                        .font(.system(size: 14))
                        .foregroundStyle(CRTheme.brandElectric)
                }
                .buttonStyle(.plain)
                .padding(.leading, 8)
                .crHoverScale(scale: 1.05)
            }
        }
        .padding(.vertical, 12)
        .contentShape(Rectangle())
    }
}
