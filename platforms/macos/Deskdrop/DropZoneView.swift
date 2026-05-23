import SwiftUI
import UniformTypeIdentifiers

struct DropZoneView: View {
    @State private var isTargeted = false
    let store: DeskdropStore
    
    var body: some View {
        VStack(spacing: 16) {
            ZStack {
                Circle()
                    .fill(Color.white.opacity(isTargeted ? 0.3 : 0.1))
                    .frame(width: 80, height: 80)
                
                Image(systemName: "square.and.arrow.down.fill")
                    .font(.system(size: 32, weight: .bold))
                    .foregroundColor(.white)
                    .offset(y: isTargeted ? 5 : 0)
                    .animation(.spring(response: 0.3, dampingFraction: 0.6), value: isTargeted)
            }
            
            Text("Drop to send to Android")
                .font(.system(size: 16, weight: .bold, design: .rounded))
                .foregroundColor(.white)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(
            CRHUDMaterial()
                .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 24, style: .continuous)
                .strokeBorder(
                    isTargeted ? Color.white : Color.white.opacity(0.2),
                    lineWidth: isTargeted ? 3 : 1
                )
        )
        .scaleEffect(isTargeted ? 1.05 : 1.0)
        .animation(.spring(response: 0.3, dampingFraction: 0.6), value: isTargeted)
        .onDrop(of: [.fileURL], isTargeted: $isTargeted) { providers in
            var handled = false
            for provider in providers {
                provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, error in
                    if let data = item as? Data,
                       let urlString = String(data: data, encoding: .utf8),
                       let url = URL(string: urlString) {
                        DispatchQueue.main.async {
                            self.store.sendFiles(urls: [url])
                            // Force hide drop zone when a file is successfully dropped
                            GlobalDragMonitor.shared.isDraggingFile = false
                        }
                    } else if let url = item as? URL {
                        DispatchQueue.main.async {
                            self.store.sendFiles(urls: [url])
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
