import Cocoa
import Combine

class GlobalDragMonitor: ObservableObject {
    static let shared = GlobalDragMonitor()
    
    @Published var isDraggingFile = false
    
    private var globalMonitor: Any?
    private var localMonitor: Any?
    private var lastChangeCount: Int = 0
    private let dragPasteboard = NSPasteboard(name: .drag)
    
    private init() {}
    
    func startMonitoring() {
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        let _ = AXIsProcessTrustedWithOptions(options)
        
        lastChangeCount = dragPasteboard.changeCount
        
        globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDragged, .leftMouseUp]) { [weak self] event in
            self?.handleEvent(event)
        }
        
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDragged, .leftMouseUp]) { [weak self] event in
            self?.handleEvent(event)
            return event
        }
    }
    
    private func handleEvent(_ event: NSEvent) {
        if event.type == .leftMouseDragged {
            let currentChangeCount = dragPasteboard.changeCount
            if currentChangeCount != lastChangeCount {
                lastChangeCount = currentChangeCount
                
                if let types = dragPasteboard.types, types.contains(.fileURL) {
                    if !isDraggingFile {
                        isDraggingFile = true
                    }
                }
            }
        } else if event.type == .leftMouseUp {
            if isDraggingFile {
                isDraggingFile = false
                // Reset change count so the next drag is reliably detected
                lastChangeCount = dragPasteboard.changeCount
            }
        }
    }
}
