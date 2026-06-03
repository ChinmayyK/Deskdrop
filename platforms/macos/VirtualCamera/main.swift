import Foundation
import CoreMediaIO
import os.log

fileprivate let logger = OSLog(subsystem: "com.deskdrop.VirtualCamera", category: "main")

// Wait for the provider source to start
let providerSource = ProviderSource(clientQueue: nil)
CMIOExtensionProvider.startService(provider: providerSource.provider)

CFRunLoopRun()
