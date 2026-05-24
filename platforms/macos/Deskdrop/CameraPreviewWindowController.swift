import AppKit
import SwiftUI

class CameraPreviewWindowController: NSWindowController {
    
    static let shared = CameraPreviewWindowController()
    
    private var imageView: NSImageView!
    private var labelView: NSTextField!
    
    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 640, height: 480),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Continuity Camera Preview"
        window.isReleasedWhenClosed = false
        window.center()
        
        super.init(window: window)
        setupUI()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    private func setupUI() {
        guard let window = window else { return }
        let contentView = NSView(frame: window.contentRect(forFrameRect: window.frame))
        contentView.wantsLayer = true
        contentView.layer?.backgroundColor = NSColor.black.cgColor
        
        imageView = NSImageView(frame: contentView.bounds)
        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.autoresizingMask = [.width, .height]
        contentView.addSubview(imageView)
        
        labelView = NSTextField(labelWithString: "Waiting for video stream...")
        labelView.textColor = .white
        labelView.font = .systemFont(ofSize: 18, weight: .medium)
        labelView.alignment = .center
        labelView.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(labelView)
        
        NSLayoutConstraint.activate([
            labelView.centerXAnchor.constraint(equalTo: contentView.centerXAnchor),
            labelView.centerYAnchor.constraint(equalTo: contentView.centerYAnchor)
        ])
        
        window.contentView = contentView
    }
    
    func updateFrame(data: Data) {
        DispatchQueue.main.async {
            self.labelView.isHidden = true
            if let image = NSImage(data: data) {
                self.imageView.image = image
            }
        }
    }
}
