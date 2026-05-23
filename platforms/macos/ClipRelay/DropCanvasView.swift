import SwiftUI
import UniformTypeIdentifiers

struct DropCanvasView: View {
    @ObservedObject var store: DeskdropStore
    @State private var isTargeted = false

    var body: some View {
        VStack(spacing: 20) {
            ZStack {
                // Background shape
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(CRTheme.brandElectric.opacity(isTargeted ? 0.15 : 0.05))
                    .overlay(
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .strokeBorder(
                                isTargeted ? CRTheme.brandElectric : CRTheme.stroke.opacity(0.5),
                                style: StrokeStyle(lineWidth: isTargeted ? 2 : 1, dash: isTargeted ? [8] : [])
                            )
                    )

                VStack(spacing: 12) {
                    Image(systemName: "arrow.down.doc.fill")
                        .font(.system(size: 48))
                        .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.inkSubtle)
                        .scaleEffect(isTargeted ? 1.1 : 1.0)
                        .animation(.crSpring, value: isTargeted)

                    VStack(spacing: 4) {
                        Text(isTargeted ? "Drop to Send" : "Drop Files Here")
                            .font(.system(size: 16, weight: .bold))
                            .foregroundStyle(CRTheme.ink)

                        Text("Sends to all connected devices")
                            .font(.system(size: 12))
                            .foregroundStyle(CRTheme.inkSoft)
                    }
                }
            }
            .frame(height: 180)
            .padding(16)
        }
        .frame(width: 320)
        .background(CRHUDMaterial().ignoresSafeArea())
        .onDrop(of: [.fileURL], delegate: CanvasDropDelegate(store: store, isTargeted: $isTargeted))
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
