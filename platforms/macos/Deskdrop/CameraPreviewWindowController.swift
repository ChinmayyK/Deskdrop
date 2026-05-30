import AppKit
import SwiftUI

class CameraStreamState: ObservableObject {
    @Published var image: NSImage? = nil
    @Published var isWaiting: Bool = true
}

struct CameraPreviewView: View {
    @ObservedObject var state: CameraStreamState
    var onClose: () -> Void
    
    var body: some View {
        ZStack {
            // Background
            VisualEffectView(material: .hudWindow, blendingMode: .behindWindow)
                .edgesIgnoringSafeArea(.all)
            
            if let img = state.image {
                Image(nsImage: img)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            
            if state.isWaiting {
                VStack(spacing: 16) {
                    ProgressView()
                        .progressViewStyle(CircularProgressViewStyle(tint: .white))
                        .scaleEffect(1.5)
                    Text("Waiting for video stream...")
                        .font(.system(size: 18, weight: .medium, design: .rounded))
                        .foregroundColor(.white)
                }
            }
            
            // Floating bottom bar
            VStack {
                Spacer()
                Button(action: onClose) {
                    HStack {
                        Image(systemName: "xmark.circle.fill")
                        Text("Stop Viewing")
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(Color.black.opacity(0.6))
                    .foregroundColor(.white)
                    .clipShape(Capsule())
                }
                .buttonStyle(PlainButtonStyle())
                .padding(.bottom, 24)
            }
        }
    }
}

struct VisualEffectView: NSViewRepresentable {
    let material: NSVisualEffectView.Material
    let blendingMode: NSVisualEffectView.BlendingMode
    
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = material
        view.blendingMode = blendingMode
        view.state = .active
        return view
    }
    
    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {}
}

class CameraPreviewWindowController: NSWindowController, NSWindowDelegate {
    
    static let shared = CameraPreviewWindowController()
    
    private var streamState = CameraStreamState()
    
    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 640, height: 480),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Continuity Camera"
        window.isReleasedWhenClosed = false
        window.center()
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        
        super.init(window: window)
        window.delegate = self
        
        let rootView = CameraPreviewView(state: streamState, onClose: { [weak self] in
            self?.close()
        })
        
        window.contentView = NSHostingView(rootView: rootView)
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    func windowWillClose(_ notification: Notification) {
        streamState.isWaiting = true
        streamState.image = nil
        NotificationCenter.default.post(name: .deskdropCameraWindowClosed, object: nil)
    }
    
    func updateFrame(data: Data) {
        DispatchQueue.main.async {
            self.streamState.isWaiting = false
            if let image = NSImage(data: data) {
                self.streamState.image = image
            }
        }
    }
}

extension Notification.Name {
    static let deskdropCameraWindowClosed = Notification.Name("deskdropCameraWindowClosed")
}
