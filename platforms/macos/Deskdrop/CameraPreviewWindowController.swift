import AppKit
import SwiftUI

class CameraStreamState: ObservableObject {
    @Published var image: NSImage? = nil
    @Published var isWaiting: Bool = true
    @Published var pulse: Bool = false
}

struct CameraPreviewView: View {
    @ObservedObject var state: CameraStreamState
    var onClose: () -> Void
    @State private var isHoveringClose = false
    
    var body: some View {
        ZStack {
            // Premium glass background
            VisualEffectView(material: .hudWindow, blendingMode: .behindWindow)
                .edgesIgnoringSafeArea(.all)
            
            if let img = state.image {
                Image(nsImage: img)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .padding(8)
                    .shadow(color: Color.black.opacity(0.3), radius: 20, x: 0, y: 10)
                    .transition(.opacity.combined(with: .scale(scale: 0.95)))
            }
            
            if state.isWaiting {
                VStack(spacing: 24) {
                    ZStack {
                        Circle()
                            .stroke(Color.white.opacity(0.2), lineWidth: 4)
                            .frame(width: 64, height: 64)
                        
                        Circle()
                            .trim(from: 0, to: 0.7)
                            .stroke(Color.white, style: StrokeStyle(lineWidth: 4, lineCap: .round))
                            .frame(width: 64, height: 64)
                            .rotationEffect(Angle(degrees: state.pulse ? 360 : 0))
                            .animation(Animation.linear(duration: 1).repeatForever(autoreverses: false), value: state.pulse)
                    }
                    .onAppear { state.pulse = true }
                    
                    Text("Connecting to Camera...")
                        .font(.system(size: 16, weight: .semibold, design: .rounded))
                        .foregroundColor(Color.white.opacity(0.9))
                        .shadow(color: .black.opacity(0.5), radius: 2, x: 0, y: 1)
                }
                .transition(.opacity)
            }
            
            // Floating control bar
            VStack {
                Spacer()
                Button(action: onClose) {
                    HStack(spacing: 8) {
                        Image(systemName: "video.slash.fill")
                            .font(.system(size: 14, weight: .bold))
                        Text("Stop Streaming")
                            .font(.system(size: 14, weight: .bold, design: .rounded))
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 12)
                    .background(
                        isHoveringClose
                        ? LinearGradient(colors: [Color.red.opacity(0.9), Color.pink.opacity(0.9)], startPoint: .topLeading, endPoint: .bottomTrailing)
                        : LinearGradient(colors: [Color.black.opacity(0.7), Color.black.opacity(0.5)], startPoint: .top, endPoint: .bottom)
                    )
                    .overlay(
                        Capsule().stroke(Color.white.opacity(isHoveringClose ? 0.3 : 0.1), lineWidth: 1)
                    )
                    .foregroundColor(.white)
                    .clipShape(Capsule())
                    .shadow(color: isHoveringClose ? Color.red.opacity(0.4) : Color.black.opacity(0.3), radius: 8, x: 0, y: 4)
                    .scaleEffect(isHoveringClose ? 1.05 : 1.0)
                    .animation(.spring(response: 0.3, dampingFraction: 0.6), value: isHoveringClose)
                }
                .buttonStyle(PlainButtonStyle())
                .onHover { hovering in
                    isHoveringClose = hovering
                }
                .padding(.bottom, 32)
            }
        }
        .animation(.easeInOut(duration: 0.3), value: state.isWaiting)
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
