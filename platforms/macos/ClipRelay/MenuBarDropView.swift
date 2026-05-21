// MenuBarDropView.swift
// Custom drag-and-drop view for the Deskdrop menu bar icon.
//
// Replaces the default NSStatusBarButton so Finder files can be dragged
// directly onto the menu bar icon and sent to all connected peers.
// Supports: multiple files, handles highlighting state, falls back to
// showing the menu on a regular click.

import AppKit
import UniformTypeIdentifiers

// MARK: - Delegate protocol

protocol MenuBarDropViewDelegate: AnyObject {
    /// Called when the user drops one or more file URLs onto the menu bar icon.
    func menuBarDropView(_ view: MenuBarDropView, didReceiveFiles urls: [URL])
    /// Called on a left-click (open menu) or right-click.
    func menuBarDropViewDidClick(_ view: MenuBarDropView)
}

// MARK: - MenuBarDropView

final class MenuBarDropView: NSView {

    weak var delegate: MenuBarDropViewDelegate?

    // Image shown in the button
    var iconImage: NSImage? {
        didSet { imageView.image = iconImage }
    }

    // Whether to overlay a small red badge
    var badgeCount: Int = 0 {
        didSet { needsDisplay = true }
    }

    // Visual state
    private var isDragHighlighted = false {
        didSet { needsDisplay = true }
    }

    // MARK: Sub-views

    private let imageView = NSImageView()

    // MARK: Init

    override init(frame: NSRect) {
        super.init(frame: frame)
        commonInit()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        commonInit()
    }

    private func commonInit() {
        wantsLayer = true
        layer?.cornerRadius = 5

        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(imageView)
        NSLayoutConstraint.activate([
            imageView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 2),
            imageView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -2),
            imageView.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            imageView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -2),
        ])

        // Accept file URLs + generic file-promise types
        registerForDraggedTypes([
            .fileURL,
            .init(rawValue: "com.apple.pasteboard.promised-file-url"),
            .init(rawValue: "com.apple.NSFilePromiseItemMetaData"),
        ])
    }

    // MARK: - Drawing

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        if isDragHighlighted {
            NSColor.controlAccentColor.withAlphaComponent(0.25).setFill()
            NSBezierPath(roundedRect: bounds, xRadius: 5, yRadius: 5).fill()
        }
        // Badge
        if badgeCount > 0 {
            let dotSize: CGFloat = bounds.height * 0.36
            let dotRect = CGRect(
                x: bounds.width - dotSize - 0.5,
                y: bounds.height - dotSize - 0.5,
                width: dotSize, height: dotSize
            )
            NSColor.systemRed.setFill()
            NSBezierPath(ovalIn: dotRect).fill()

            if badgeCount > 1 {
                let label = badgeCount < 10 ? "\(badgeCount)" : "+"
                let attrs: [NSAttributedString.Key: Any] = [
                    .font: NSFont.systemFont(ofSize: dotSize * 0.68, weight: .bold),
                    .foregroundColor: NSColor.white
                ]
                let str = NSAttributedString(string: label, attributes: attrs)
                let sz = str.size()
                str.draw(at: CGPoint(x: dotRect.midX - sz.width / 2, y: dotRect.midY - sz.height / 2))
            }
        }
    }

    // MARK: - Mouse handling

    override func mouseDown(with event: NSEvent) {
        delegate?.menuBarDropViewDidClick(self)
    }

    // MARK: - Drag destination

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        let hasFiles = sender.draggingPasteboard.canReadObject(forClasses: [NSURL.self], options: [
            .urlReadingFileURLsOnly: true
        ])
        if hasFiles {
            isDragHighlighted = true
            return .copy
        }
        return []
    }

    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        let hasFiles = sender.draggingPasteboard.canReadObject(forClasses: [NSURL.self], options: [
            .urlReadingFileURLsOnly: true
        ])
        return hasFiles ? .copy : []
    }

    override func draggingExited(_ sender: NSDraggingInfo?) {
        isDragHighlighted = false
    }

    override func draggingEnded(_ sender: NSDraggingInfo) {
        isDragHighlighted = false
    }

    override func prepareForDragOperation(_ sender: NSDraggingInfo) -> Bool { true }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        let pb = sender.draggingPasteboard
        guard let urls = pb.readObjects(forClasses: [NSURL.self], options: [
            .urlReadingFileURLsOnly: true
        ]) as? [URL], !urls.isEmpty else { return false }

        delegate?.menuBarDropView(self, didReceiveFiles: urls)
        return true
    }
}
