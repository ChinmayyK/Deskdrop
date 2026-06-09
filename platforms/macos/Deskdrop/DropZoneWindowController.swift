import AppKit
import SwiftUI

final class DropZoneWindowController: NSWindowController {
    let store: DeskdropStore
    
    init(store: DeskdropStore) {
        self.store = store
        
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 440, height: 380),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .popUpMenu // So it appears above standard windows
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.center()
        
        super.init(window: panel)
        
        let rootView = DropZoneView(store: store, onClose: { [weak self] in
            self?.closeWindow()
        })
        
        panel.contentViewController = NSHostingController(rootView: rootView)
        
        // Listen for Esc key to close
        NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            if event.keyCode == 53 { // Escape
                self?.closeWindow()
                return nil // Consume event
            }
            return event
        }
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    func show() {
        guard let window = self.window else { return }
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
    
    func closeWindow() {
        window?.orderOut(nil)
    }
}
