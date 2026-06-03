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
    
    func startStream() throws {
        guard !isStreaming else { return }
        isStreaming = true
        
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
        
        // Fill with a solid color (e.g. green) as a placeholder
        if let baseAddress = baseAddress {
            memset(baseAddress, 0x88, height * bytesPerRow)
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
