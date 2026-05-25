import Cocoa
import Combine

class GlobalDragMonitor: ObservableObject {
    static let shared = GlobalDragMonitor()
    
    @Published var isDraggingFile = false
    
    private var timer: Timer?
    private var lastChangeCount: Int = 0
    private let dragPasteboard = NSPasteboard(name: .drag)
    
    private init() {}
    
    func startMonitoring() {
        lastChangeCount = dragPasteboard.changeCount
        
        // Use a high-frequency timer on the common runloop to poll for drags.
        // This completely eliminates the need for Accessibility permissions.
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            self?.checkDragState()
        }
        RunLoop.main.add(timer!, forMode: .common)
    }
    
    private func checkDragState() {
        let currentChangeCount = dragPasteboard.changeCount
        
        // 1. Detect if a new drag started
        if currentChangeCount != lastChangeCount {
            lastChangeCount = currentChangeCount
            
            if let types = dragPasteboard.types, types.contains(.fileURL) {
                if !isDraggingFile {
                    isDraggingFile = true
                }
            }
        }
        
        // 2. Detect if drag has ended
        if isDraggingFile {
            // NSEvent.pressedMouseButtons queries hardware state (0 = no buttons pressed)
            if NSEvent.pressedMouseButtons == 0 {
                isDraggingFile = false
                // Reset so the next drag doesn't falsely trigger
                lastChangeCount = dragPasteboard.changeCount 
            }
        }
    }
}
