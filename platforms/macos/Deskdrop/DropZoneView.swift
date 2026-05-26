import SwiftUI
import UniformTypeIdentifiers

struct DropZoneView: View {
    @State private var isTargeted = false
    @State private var pulse = false
    let store: DeskdropStore
    
    var body: some View {
        ZStack {
            // Pulse glow when targeted
            if isTargeted {
                CRTheme.brandElectric
                    .opacity(0.2)
                    .blur(radius: 50)
                    .ignoresSafeArea()
            }
            
            VStack(spacing: 20) {
                ZStack {
                    Circle()
                        .fill(isTargeted ? CRTheme.brandElectric.opacity(0.25) : Color.white.opacity(0.1))
                        .frame(width: 88, height: 88)
                        .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.5) : .clear, radius: 12)
                    
                    Image(systemName: "square.and.arrow.down.fill")
                        .font(.system(size: 38, weight: .semibold))
                        .foregroundColor(isTargeted ? .white : Color.white.opacity(0.8))
                        .offset(y: isTargeted ? 8 : -2)
                }
                .scaleEffect(isTargeted ? 1.15 : 1.0)
                .animation(.crSpring, value: isTargeted)
                
                VStack(spacing: 6) {
                    Text(isTargeted ? "Release to Send" : "Drop to Send")
                        .font(.system(size: 18, weight: .bold, design: .rounded))
                        .foregroundColor(.white)
                        .contentTransition(.interpolate)
                    
                    Text(store.defaultTargetDevice != nil ? "Sends to \(store.defaultTargetDevice!.name)" : "Sends to all connected devices")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(Color.white.opacity(0.6))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(
                CRHUDMaterial()
                    .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 24, style: .continuous)
                    .strokeBorder(
                        isTargeted ? CRTheme.brandElectric : Color.white.opacity(0.15),
                        lineWidth: isTargeted ? 3 : 1.5
                    )
                    .shadow(color: isTargeted ? CRTheme.brandElectric.opacity(0.3) : .clear, radius: 8)
            )
            .shadow(color: Color.black.opacity(0.3), radius: 24, x: 0, y: 12)
            .scaleEffect(isTargeted ? 1.02 : 1.0)
            .animation(.crSpring, value: isTargeted)
            .onChange(of: isTargeted) { targeted in
                if targeted {
                    withAnimation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true)) { pulse = true }
                } else {
                    withAnimation(.easeOut(duration: 0.3)) { pulse = false }
                }
            }
        }
        .padding(16) // Padding for the shadow and glow effects to breathe
        .onDrop(of: [.fileURL], isTargeted: $isTargeted) { providers in
            var handled = false
            for provider in providers {
                provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, error in
                    if let data = item as? Data,
                       let urlString = String(data: data, encoding: .utf8),
                       let url = URL(string: urlString) {
                        DispatchQueue.main.async {
                            self.store.sendFiles(urls: [url], to: self.store.defaultTargetDevice)
                            // Force hide drop zone when a file is successfully dropped
                            GlobalDragMonitor.shared.isDraggingFile = false
                        }
                    } else if let url = item as? URL {
                        DispatchQueue.main.async {
                            self.store.sendFiles(urls: [url], to: self.store.defaultTargetDevice)
                            GlobalDragMonitor.shared.isDraggingFile = false
                        }
                    }
                }
                handled = true
            }
            return handled
        }
    }
}

// Removed preview since store is required and mock is not easily available
