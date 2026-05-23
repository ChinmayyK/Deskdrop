import Cocoa
import SwiftUI
import Combine

class DropZoneWindowController: NSWindowController {
    private var windowCancelable: AnyCancellable?
    
    init(store: DeskdropStore) {
        let screenRect = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 800, height: 600)
        let width: CGFloat = 280
        let height: CGFloat = 200
        
        // Position at the bottom center of the screen, just above the dock
        let rect = NSRect(
            x: screenRect.midX - (width / 2),
            y: screenRect.minY + 40,
            width: width,
            height: height
        )
        
        let window = NSWindow(
            contentRect: rect,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        
        window.level = .floating
        window.backgroundColor = .clear
        window.isOpaque = false
        window.hasShadow = true
        window.ignoresMouseEvents = false // Important so it can receive drops
        window.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        
        let rootView = DropZoneView(store: store)
        window.contentView = NSHostingView(rootView: rootView)
        
        super.init(window: window)
        
        self.window?.alphaValue = 0.0
        
        // Observe dragging state
        windowCancelable = GlobalDragMonitor.shared.$isDraggingFile
            .receive(on: RunLoop.main)
            .sink { [weak self] isDragging in
                if isDragging {
                    self?.showWindow(nil)
                    NSAnimationContext.runAnimationGroup { context in
                        context.duration = 0.3
                        context.timingFunction = CAMediaTimingFunction(name: .easeOut)
                        self?.window?.animator().alphaValue = 1.0
                    }
                } else {
                    NSAnimationContext.runAnimationGroup { context in
                        context.duration = 0.3
                        context.timingFunction = CAMediaTimingFunction(name: .easeIn)
                        self?.window?.animator().alphaValue = 0.0
                    } completionHandler: {
                        self?.window?.orderOut(nil)
                    }
                }
            }
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
