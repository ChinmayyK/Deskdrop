import Foundation
import CoreMediaIO
import CoreVideo
import os.log

class StreamSource: NSObject, CMIOExtensionStreamSource {
    private(set) var stream: CMIOExtensionStream!
    private var pixelBufferPool: CVPixelBufferPool?
    private var isStreaming = false
    private let videoDimensions: CMVideoDimensions
    private var timer: Timer?
    
    // CMIOExtensionStreamSource properties
    private(set) var formats: [CMIOExtensionStreamFormat] = []
    private(set) var activeFormatIndex: Int = 0
    
    init(localizedName: String, videoDimensions: CMVideoDimensions) {
        self.videoDimensions = videoDimensions
        super.init()
        
        let streamID = UUID()
        stream = CMIOExtensionStream(localizedName: localizedName, streamID: streamID, direction: .source, clockType: .hostTime, source: self)
        
        if let format = createFormatDescription(dimensions: videoDimensions) {
            let streamFormat = CMIOExtensionStreamFormat(formatDescription: format, maxFrameDuration: CMTime(value: 1, timescale: 30), minFrameDuration: CMTime(value: 1, timescale: 30), validFrameDurations: nil)
            self.formats = [streamFormat]
            self.activeFormatIndex = 0
        }
        
        setupPixelBufferPool()
    }
    
    private func createFormatDescription(dimensions: CMVideoDimensions) -> CMFormatDescription? {
        var formatDescription: CMFormatDescription?
        CMVideoFormatDescriptionCreate(allocator: kCFAllocatorDefault,
                                       codecType: kCVPixelFormatType_32BGRA,
                                       width: dimensions.width,
                                       height: dimensions.height,
                                       extensions: nil,
                                       formatDescriptionOut: &formatDescription)
        return formatDescription
    }
    
    private func setupPixelBufferPool() {
        let pixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: videoDimensions.width,
            kCVPixelBufferHeightKey as String: videoDimensions.height,
            kCVPixelBufferIOSurfacePropertiesKey as String: [:]
        ]
        
        CVPixelBufferPoolCreate(kCFAllocatorDefault, nil, pixelBufferAttributes as CFDictionary, &pixelBufferPool)
    }
    
    var availableProperties: Set<CMIOExtensionProperty> {
        return [.streamActiveFormatIndex]
    }
    
    func streamProperties(forProperties properties: Set<CMIOExtensionProperty>) throws -> CMIOExtensionStreamProperties {
        let streamProperties = CMIOExtensionStreamProperties(dictionary: [:])
        if properties.contains(.streamActiveFormatIndex) {
            streamProperties.setPropertyState(CMIOExtensionPropertyState(value: 0 as NSNumber), forProperty: .streamActiveFormatIndex)
        }
        return streamProperties
    }
    
    func setStreamProperties(_ streamProperties: CMIOExtensionStreamProperties) throws {
        // Read-only
    }
    
    func authorizedToStartStream(for client: CMIOExtensionClient) -> Bool {
        return true
    }
    
    // Image fetching state
    private var latestFrame: CGImage?
    private var fetchTask: Task<Void, Never>?
    
    func startStream() throws {
        guard !isStreaming else { return }
        isStreaming = true
        
        // Start background fetch loop
        fetchTask = Task {
            while !Task.isCancelled {
                do {
                    let (data, _) = try await URLSession.shared.data(from: URL(string: "http://127.0.0.1:40404/")!)
                    if let imageSource = CGImageSourceCreateWithData(data as CFData, nil),
                       let cgImage = CGImageSourceCreateImageAtIndex(imageSource, 0, nil) {
                        self.latestFrame = cgImage
                    }
                } catch {
                    // Ignore fetch errors (daemon might be down or no frame yet)
                    try? await Task.sleep(nanoseconds: 100_000_000)
                }
                try? await Task.sleep(nanoseconds: 16_000_000) // ~60fps poll rate
            }
        }
        
        // Timer to generate 30fps frames
        DispatchQueue.main.async {
            self.timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] _ in
                self?.generateFrame()
            }
        }
    }
    
    func stopStream() throws {
        guard isStreaming else { return }
        isStreaming = false
        
        fetchTask?.cancel()
        fetchTask = nil
        
        DispatchQueue.main.async {
            self.timer?.invalidate()
            self.timer = nil
        }
    }
    
    private func generateFrame() {
        guard let pool = pixelBufferPool else { return }
        var pixelBuffer: CVPixelBuffer?
        CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &pixelBuffer)
        
        guard let buffer = pixelBuffer else { return }
        
        CVPixelBufferLockBaseAddress(buffer, [])
        let baseAddress = CVPixelBufferGetBaseAddress(buffer)
        let width = CVPixelBufferGetWidth(buffer)
        let height = CVPixelBufferGetHeight(buffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(buffer)
        
        if let baseAddress = baseAddress {
            if let cgImage = latestFrame {
                // Render CGImage onto the pixel buffer
                let colorSpace = CGColorSpaceCreateDeviceRGB()
                if let context = CGContext(data: baseAddress,
                                           width: width,
                                           height: height,
                                           bitsPerComponent: 8,
                                           bytesPerRow: bytesPerRow,
                                           space: colorSpace,
                                           bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue) {
                    
                    // Clear background
                    context.clear(CGRect(x: 0, y: 0, width: width, height: height))
                    
                    // Draw centered maintaining aspect ratio
                    let imgWidth = CGFloat(cgImage.width)
                    let imgHeight = CGFloat(cgImage.height)
                    let targetRatio = CGFloat(width) / CGFloat(height)
                    let imgRatio = imgWidth / imgHeight
                    
                    var drawRect = CGRect.zero
                    if imgRatio > targetRatio {
                        let newHeight = CGFloat(width) / imgRatio
                        drawRect = CGRect(x: 0, y: (CGFloat(height) - newHeight) / 2.0, width: CGFloat(width), height: newHeight)
                    } else {
                        let newWidth = CGFloat(height) * imgRatio
                        drawRect = CGRect(x: (CGFloat(width) - newWidth) / 2.0, y: 0, width: newWidth, height: CGFloat(height))
                    }
                    
                    context.draw(cgImage, in: drawRect)
                }
            } else {
                // Fill with a dark gray placeholder if no frame is available yet
                memset(baseAddress, 0x11, height * bytesPerRow)
            }
        }
        CVPixelBufferUnlockBaseAddress(buffer, [])
        
        // Create CMSampleBuffer
        var sampleBuffer: CMSampleBuffer?
        var timingInfo = CMSampleTimingInfo(duration: CMTime(value: 1, timescale: 30),
                                            presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()),
                                            decodeTimeStamp: .invalid)
        
        if let formatDesc = self.formats.first?.formatDescription {
            CMSampleBufferCreateReadyWithImageBuffer(allocator: kCFAllocatorDefault,
                                                     imageBuffer: buffer,
                                                     formatDescription: formatDesc,
                                                     sampleTiming: &timingInfo,
                                                     sampleBufferOut: &sampleBuffer)
        }
        
        if let sampleBuffer = sampleBuffer {
            stream.send(sampleBuffer, discontinuity: .time, hostTimeInNanoseconds: UInt64(timingInfo.presentationTimeStamp.seconds * 1_000_000_000))
        }
    }
}
