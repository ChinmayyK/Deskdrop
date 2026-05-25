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
                CRTheme.brandElectric
                    .opacity(0.15)
                    .blur(radius: 40)
                    .ignoresSafeArea()
            }

            VStack(spacing: 20) {
                ZStack {
                    // Background shape
                    RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .fill(CRTheme.surfaceElevated.opacity(0.4))
                        .overlay(
                            RoundedRectangle(cornerRadius: 16, style: .continuous)
                                .strokeBorder(
                                    isTargeted ? CRTheme.brandElectric : CRTheme.stroke,
                                    lineWidth: isTargeted ? 2 : 1
                                )
                                .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.4) : .clear, radius: 8, x: 0, y: 0)
                        )

                    VStack(spacing: 12) {
                        ZStack {
                            Circle()
                                .fill(isTargeted ? CRTheme.brandElectric.opacity(0.2) : CRTheme.strokeSoft)
                                .frame(width: 64, height: 64)
                            
                            Image(systemName: "arrow.down.doc.fill")
                                .font(.system(size: 28))
                                .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.inkSoft)
                                .offset(y: isTargeted ? 2 : -2)
                        }
                        .scaleEffect(isTargeted ? 1.1 : 1.0)
                        .animation(.crSpring, value: isTargeted)

                        VStack(spacing: 4) {
                            Text(isTargeted ? "Drop to Send" : "Drop Files Here")
                                .font(.system(size: 16, weight: .semibold, design: .rounded))
                                .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.ink)
                                .contentTransition(.interpolate)

                            Text("Sends to all connected devices")
                                .font(.system(size: 12))
                                .foregroundStyle(CRTheme.inkSubtle)
                        }
                    }
                }
                .frame(height: 180)
                .padding(16)
            }
        }
        .frame(width: 320, height: 212) // Slightly taller to fit padding properly
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
        
        let providers = info.itemProviders(for: [.fileURL])
        let group = DispatchGroup()
        var urls: [URL] = []
        
        for provider in providers {
            group.enter()
            provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { (item, error) in
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
