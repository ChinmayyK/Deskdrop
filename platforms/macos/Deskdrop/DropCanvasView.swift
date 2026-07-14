import SwiftUI
import UniformTypeIdentifiers

struct DropCanvasView: View {
    @ObservedObject var store: DeskdropStore
    @State private var isTargeted = false
    @State private var pulse = false

    var body: some View {
        ZStack {
            // Base layer: Native macOS Frosted Glass
            CRHUDMaterial()
                .ignoresSafeArea()
            
            // Pulse glow when targeted
            if isTargeted {
                LinearGradient(
                    colors: [CRTheme.brandElectric.opacity(0.20), CRTheme.brandViolet.opacity(0.15), CRTheme.brandCyan.opacity(0.15)],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .blur(radius: 35)
                .ignoresSafeArea()
            }

            VStack(spacing: 12) {
                // Header Status Bar
                HStack {
                    HStack(spacing: 6) {
                        Circle()
                            .fill(store.connectedCount > 0 ? CRTheme.accentGreen : CRTheme.accentOrange)
                            .frame(width: 6.5, height: 6.5)
                            .shadow(color: (store.connectedCount > 0 ? CRTheme.accentGreen : CRTheme.accentOrange).opacity(0.6), radius: 3)
                        Text(store.connectedCount > 0 ? "\(store.connectedCount) Device\(store.connectedCount == 1 ? "" : "s") Ready" : "Searching Peers...")
                            .font(.system(size: 11, weight: .semibold, design: .rounded))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(CRTheme.surfaceStrong.opacity(0.6)))
                    .overlay(Capsule().strokeBorder(CRTheme.strokeSoft, lineWidth: 0.5))
                    
                    Spacer()
                    
                    Button(action: {
                        NotificationCenter.default.post(name: .init("closeDropCanvas"), object: nil)
                    }) {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(CRTheme.inkSubtle)
                            .frame(width: 22, height: 22)
                            .background(Circle().fill(CRTheme.surfaceStrong.opacity(0.5)))
                            .overlay(Circle().strokeBorder(CRTheme.strokeSoft, lineWidth: 0.5))
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.top, 14)

                // Drop Area Card
                ZStack {
                    // Background shape
                    ZStack {
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .fill(CRTheme.surfaceElevated.opacity(isTargeted ? 0.6 : 0.4))
                            .overlay(
                                RoundedRectangle(cornerRadius: 16, style: .continuous)
                                    .strokeBorder(
                                        isTargeted ? CRTheme.brandElectric : CRTheme.stroke,
                                        lineWidth: isTargeted ? 2 : 1
                                    )
                                    .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.45) : .clear, radius: 10, x: 0, y: 0)
                            )
                        
                        if isTargeted {
                            // Animated radar rings
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(CRTheme.brandElectric, lineWidth: 2)
                                .scaleEffect(pulse ? 1.04 : 1.0)
                                .opacity(pulse ? 0 : 0.8)
                            
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .stroke(CRTheme.brandCyan, lineWidth: 1)
                                .scaleEffect(pulse ? 1.08 : 1.0)
                                .opacity(pulse ? 0 : 0.4)
                        }
                    }

                    VStack(spacing: 10) {
                        ZStack {
                            Circle()
                                .fill(isTargeted ? CRTheme.brandElectric.opacity(0.18) : CRTheme.strokeSoft)
                                .frame(width: 58, height: 58)
                                .overlay(
                                    Circle()
                                        .strokeBorder(
                                            isTargeted ? CRTheme.brandElectric : CRTheme.stroke,
                                            style: StrokeStyle(lineWidth: isTargeted ? 2 : 1.5, dash: isTargeted ? [] : [4, 3])
                                        )
                                )
                            
                            Image(systemName: isTargeted ? "arrow.down.app.fill" : "arrow.down.doc.fill")
                                .font(.system(size: 24, weight: .medium))
                                .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.inkSoft)
                                .offset(y: isTargeted ? 2 : -2)
                                .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.5) : .clear, radius: 8)
                        }
                        .scaleEffect(isTargeted ? 1.12 : 1.0)
                        .animation(.spring(response: 0.3, dampingFraction: 0.65), value: isTargeted)

                        VStack(spacing: 3) {
                            Text(isTargeted ? "Release to Broadcast ✨" : "Drop to Broadcast")
                                .font(.system(size: 15, weight: .bold, design: .rounded))
                                .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.ink)
                                .contentTransition(.interpolate)

                            Text(isTargeted ? "Sending to all connected devices instantly" : "Wireless transfer to nearby devices")
                                .font(.system(size: 11.5, weight: .medium))
                                .foregroundStyle(CRTheme.inkSubtle)
                                .contentTransition(.interpolate)
                        }
                    }
                }
                .frame(height: 140)
                .padding(.horizontal, 16)
                .padding(.bottom, 16)
            }
        }
        .frame(width: 320, height: 212)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .onDrop(of: [.fileURL], delegate: CanvasDropDelegate(store: store, isTargeted: $isTargeted))
        .onChange(of: isTargeted) { targeted in
            if targeted {
                withAnimation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true)) {
                    pulse = true
                }
            } else {
                withAnimation(.easeOut(duration: 0.3)) {
                    pulse = false
                }
            }
        }
    }
}

struct CanvasDropDelegate: DropDelegate {
    let store: DeskdropStore
    @Binding var isTargeted: Bool

    func dropEntered(info: DropInfo) {
        withAnimation(.crFast) { isTargeted = true }
    }

    func dropExited(info: DropInfo) {
        withAnimation(.crFast) { isTargeted = false }
        // Close the popover when dragging exits the canvas
        NotificationCenter.default.post(name: .init("closeDropCanvas"), object: nil)
    }

    func performDrop(info: DropInfo) -> Bool {
        isTargeted = false
        NotificationCenter.default.post(name: .init("closeDropCanvas"), object: nil)
        NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .default)
        
        let providers = info.itemProviders(for: [.fileURL])
        let group = DispatchGroup()
        let lock = NSLock()
        var urls: [URL] = []
        
        for provider in providers {
            group.enter()
            provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { (item, error) in
                lock.lock()
                defer { lock.unlock() }
                if let data = item as? Data,
                   let url = URL(dataRepresentation: data, relativeTo: nil) {
                    urls.append(url)
                } else if let url = item as? URL {
                    urls.append(url)
                }
                group.leave()
            }
        }
        
        group.notify(queue: .main) {
            if !urls.isEmpty {
                store.sendFiles(urls: urls, toPeer: nil)
                store.showToast(
                    title: "Sending \(urls.count) file\(urls.count == 1 ? "" : "s")",
                    body: urls.map(\.lastPathComponent).joined(separator: ", "),
                    tint: CRTheme.brandElectric,
                    systemImage: "arrow.up.doc.fill",
                    ttl: 3.5
                )
            }
        }
        return true
    }
}
