import SwiftUI
import UniformTypeIdentifiers

struct AnimatedAmbientBackground: View {
    @State private var phase = 0.0
    
    var body: some View {
        ZStack {
            CRHUDMaterial().ignoresSafeArea()
            
            GeometryReader { proxy in
                let size = proxy.size
                
                Circle()
                    .fill(CRTheme.brandElectric)
                    .frame(width: size.width * 0.8)
                    .blur(radius: 100)
                    .offset(
                        x: phase == 0 ? -size.width/4 : size.width/4,
                        y: phase == 0 ? -size.height/4 : size.height/4
                    )
                
                Circle()
                    .fill(CRTheme.brandViolet)
                    .frame(width: size.width * 0.7)
                    .blur(radius: 80)
                    .offset(
                        x: phase == 0 ? size.width/4 : -size.width/4,
                        y: phase == 0 ? size.height/3 : -size.height/3
                    )
            }
            .opacity(0.25)
            .onAppear {
                withAnimation(.easeInOut(duration: 6.0).repeatForever(autoreverses: true)) {
                    phase = 1.0
                }
            }
        }
    }
}

struct DropZoneView: View {
    @ObservedObject var store: DeskdropStore
    let onClose: () -> Void
    
    @State private var isTargeted = false
    @State private var pulse = false
    @State private var bounce = false
    
    var body: some View {
        ZStack {
            // The entire window background is transparent
            Color.clear
            
            // The Glassy Floating Drop Pod
            ZStack {
                // Glass Base
                RoundedRectangle(cornerRadius: 36, style: .continuous)
                    .fill(.ultraThinMaterial)
                    .background(
                        AnimatedAmbientBackground()
                            .opacity(isTargeted ? 1.0 : 0.6)
                            .clipShape(RoundedRectangle(cornerRadius: 36, style: .continuous))
                    )
                    .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.4) : Color.black.opacity(0.15), radius: isTargeted ? 24 : 16, x: 0, y: isTargeted ? 0 : 8)
                
                if isTargeted {
                    // Glowing edge only when hovering
                    RoundedRectangle(cornerRadius: 36, style: .continuous)
                        .stroke(CRTheme.brandElectric, lineWidth: 3)
                        
                    RoundedRectangle(cornerRadius: 36, style: .continuous)
                        .stroke(CRTheme.brandElectric, lineWidth: 2)
                        .scaleEffect(pulse ? 1.06 : 1.0)
                        .opacity(pulse ? 0 : 0.6)
                } else {
                    // Very subtle glass rim (no harsh bezel)
                    RoundedRectangle(cornerRadius: 36, style: .continuous)
                        .stroke(Color.white.opacity(0.15), lineWidth: 1)
                }
                
                // Close Button
                VStack {
                    HStack {
                        Spacer()
                        Button(action: onClose) {
                            Image(systemName: "xmark.circle.fill")
                                .font(.system(size: 20))
                                .foregroundStyle(CRTheme.inkSubtle)
                        }
                        .buttonStyle(.plain)
                        .padding(16)
                    }
                    Spacer()
                }
                
                // Inner Content
                VStack(spacing: 16) {
                    ZStack {
                        Circle()
                            .fill(isTargeted ? CRTheme.brandElectric.opacity(0.2) : .clear)
                            .frame(width: 90, height: 90)
                        
                        Image(systemName: "arrow.down.doc.fill")
                            .font(.system(size: 36))
                            .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.inkSoft)
                            .offset(y: isTargeted && bounce ? 6 : -6)
                            .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.5) : .clear, radius: 8, x: 0, y: 4)
                    }
                    .scaleEffect(isTargeted ? 1.1 : 1.0)
                    .animation(.crSpring, value: isTargeted)
                    
                    VStack(spacing: 4) {
                        Text(isTargeted ? "Release to Send" : "Drop Files")
                            .font(.system(size: 28, weight: .bold, design: .rounded))
                            .foregroundStyle(isTargeted ? CRTheme.brandElectric : CRTheme.ink)
                            .contentTransition(.interpolate)
                        
                        Text("Instantly send to all devices")
                            .font(.system(size: 14, weight: .medium, design: .rounded))
                            .foregroundStyle(CRTheme.inkSubtle)
                    }
                }
            }
            .frame(width: 320, height: 260)
        }
        .frame(width: 440, height: 380)
        .onDrop(of: [.fileURL], delegate: ZoneDropDelegate(store: store, isTargeted: $isTargeted, onClose: onClose))
        .onChange(of: isTargeted) { targeted in
            if targeted {
                withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) {
                    pulse = true
                }
                withAnimation(.easeInOut(duration: 0.6).repeatForever(autoreverses: true)) {
                    bounce = true
                }
            } else {
                withAnimation(.easeOut(duration: 0.3)) {
                    pulse = false
                    bounce = false
                }
            }
        }
    }
}

struct ZoneDropDelegate: DropDelegate {
    let store: DeskdropStore
    @Binding var isTargeted: Bool
    let onClose: () -> Void
    
    func dropEntered(info: DropInfo) {
        withAnimation(.crFast) { isTargeted = true }
    }
    
    func dropExited(info: DropInfo) {
        withAnimation(.crFast) { isTargeted = false }
    }
    
    func performDrop(info: DropInfo) -> Bool {
        isTargeted = false
        NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .default)
        
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
            onClose()
        }
        return true
    }
}
