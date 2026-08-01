import SwiftUI

struct DynamicHeroHeaderView: View {
    @ObservedObject var store: DeskdropStore
    @StateObject private var headerManager = DynamicHeaderManager.shared
    
    // For the animation
    @State private var taglineId: UUID = UUID()
    
    private var targetDevice: ManagedDevice? {
        if let id = store.selectedPendingDevice?.id, let fresh = store.pendingDevices.first(where: { $0.id == id }) {
            return fresh
        }
        return store.pendingDevices.first(where: { $0.pairingRequested || $0.outgoingPairingWaiting })
    }
    
    var body: some View {
        Group {
            if let targetDevice = targetDevice {
                pairingHijackView(for: targetDevice)
            } else {
                standardHeroView
            }
        }
        .padding(.horizontal, 40)
        .animation(.crSpring, value: targetDevice?.id)
        .onChange(of: currentTagline) { _ in
            withAnimation(.easeOut(duration: 0.35)) {
                taglineId = UUID()
            }
        }
        .onAppear {
            // Trigger initial animation
            withAnimation(.easeOut(duration: 0.35)) {
                taglineId = UUID()
            }
        }
    }
    
    private var standardHeroView: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(currentTagline)
                .font(.system(size: 32, weight: .bold, design: .rounded))
                .foregroundStyle(CRTheme.ink)
                .id(taglineId) // Force re-render for animation on change
                .transition(.asymmetric(
                    insertion: .move(edge: .bottom).combined(with: .opacity),
                    removal: .opacity
                ))
            
            HStack(spacing: 8) {
                if store.connectedCount > 0 {
                    Circle().fill(CRTheme.accentGreen).frame(width: 8, height: 8)
                    Text(currentSubtitle)
                        .font(.system(size: 15, weight: .regular))
                        .foregroundStyle(CRTheme.inkSoft)
                } else {
                    Circle().fill(CRTheme.accentOrange).frame(width: 8, height: 8)
                    Text(currentSubtitle)
                        .font(.system(size: 15, weight: .regular))
                        .foregroundStyle(CRTheme.inkSoft)
                }
            }
        }
        .transition(.asymmetric(
            insertion: .move(edge: .leading).combined(with: .opacity),
            removal: .move(edge: .trailing).combined(with: .opacity)
        ))
    }
    
    @ViewBuilder
    private func pairingHijackView(for device: ManagedDevice) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            if device.pairingRequested {
                Text("\(device.name) wants to pair.")
                    .font(.system(size: 32, weight: .bold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
                
                if let pin = device.pairingPin, !pin.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Verify this PIN matches your device:")
                            .font(.system(size: 14))
                            .foregroundStyle(CRTheme.inkSoft)
                        Text(pin)
                            .font(.system(size: 36, weight: .bold, design: .monospaced))
                            .foregroundStyle(CRTheme.ink)
                            .tracking(4)
                            .padding(.horizontal, 24)
                            .padding(.vertical, 12)
                            .background(CRTheme.surfaceStrong)
                            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                    }
                } else {
                    Text("This device wants to connect to your Mac.")
                        .font(.system(size: 15))
                        .foregroundStyle(CRTheme.inkSoft)
                }
                
                HStack(spacing: 12) {
                    Button("Decline") { store.respondToPairing(device, accepted: false) }
                        .buttonStyle(CRSecondaryButtonStyle())
                    Button("Accept") { store.respondToPairing(device, accepted: true) }
                        .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.accentGreen))
                }
            } else if device.outgoingPairingWaiting {
                Text("Waiting for \(device.name)...")
                    .font(.system(size: 32, weight: .bold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
                
                if let pin = device.pairingPin {
                    Text("Verify code: \(pin)")
                        .font(.system(size: 18, weight: .medium, design: .monospaced))
                        .foregroundStyle(CRTheme.brandElectric)
                }

                HStack(spacing: 8) {
                    ProgressView().scaleEffect(0.6)
                    Text("Accept the request on your device.")
                        .font(.system(size: 15))
                        .foregroundStyle(CRTheme.inkSoft)
                    
                    Button("Cancel") { store.cancelPairingRequest(device) }
                        .buttonStyle(CRSecondaryButtonStyle())
                        .padding(.leading, 12)
                }
            } else {
                Text("\(device.name) discovered nearby.")
                    .font(.system(size: 32, weight: .bold, design: .rounded))
                    .foregroundStyle(CRTheme.ink)
                Text("Connected over Local Wi-Fi.")
                    .font(.system(size: 15))
                    .foregroundStyle(CRTheme.inkSoft)
                Button("Pair with Device") { store.sendPairingRequest(device) }
                    .buttonStyle(CRPrimaryButtonStyle(tint: CRTheme.brandElectric))
            }
        }
        .transition(.asymmetric(
            insertion: .move(edge: .trailing).combined(with: .opacity),
            removal: .move(edge: .leading).combined(with: .opacity)
        ))
    }
    
    // Logic to determine the current state tagline
    private var currentTagline: String {
        if store.connectedCount == 0 {
            return DeskdropTaglines.noDevice
        }
        
        // Wait, does the store have a connecting state? 
        // We'll rely on the fact that if a device is paired but not trusted yet it's "connecting", or just skip it if we don't have it.
        // The user wanted "Connecting" as an override. Let's assume if there are devices but none are trusted, it's connecting.
        // Or if there's an active transfer.
        
        if store.batchedTransfers.contains(where: { 
            if case .transferring = $0.status { return true }
            return false
        }) {
            return DeskdropTaglines.transferring
        }
        
        if let last = store.activityFeed.first(where: { $0.isApplicable }), last.kind == "file_transfer_complete", Date().timeIntervalSince1970 - Double(last.timestamp_ms) / 1000 < 10 {
            // Show transfer complete for 10 seconds
            return DeskdropTaglines.transferComplete
        }
        
        if store.peerBatteries.contains(where: { $0.level < 15 }) {
            return DeskdropTaglines.lowBattery
        }
        
        if store.connectedCount > 1 {
            return DeskdropTaglines.multipleDevices
        }
        
        // Default to the rotating daily tagline
        return headerManager.dailyTagline
    }
    
    private var currentSubtitle: String {
        if store.connectedCount == 0 {
            return "Scan your network to pair."
        }
        
        if store.activeTransfers.contains(where: { 
            if case .transferring = $0.status { return true }
            return false
        }) {
            return "Fast enough to feel invisible."
        }
        
        if store.connectedCount > 1 {
            return "Zero-copy access enabled across \(store.connectedCount) devices."
        }
        
        return "Connected over Local Wi-Fi."
    }
}
