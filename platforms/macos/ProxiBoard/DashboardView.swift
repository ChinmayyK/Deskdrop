// ClipRelay — macOS Dashboard
// Shows connected devices with human-readable names and lifecycle controls.
// Internal device IDs are NEVER shown here.

import SwiftUI

struct DashboardView: View {
    @EnvironmentObject var store: ClipRelayStore

    var body: some View {
        VStack(spacing: 0) {
            headerView
            Divider()
            if store.peers.isEmpty {
                emptyStateView
            } else {
                ScrollView {
                    LazyVStack(spacing: 8) {
                        ForEach(store.peers) { peer in
                            DeviceCardView(peer: peer)
                        }
                    }
                    .padding(12)
                }
            }
        }
        .frame(minWidth: 340, minHeight: 300)
    }

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("ClipRelay")
                    .font(.headline)
                Text(store.statusLine)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
            Circle()
                .fill(store.isRunning ? Color.green : Color.red)
                .frame(width: 8, height: 8)
        }
        .padding(12)
    }

    private var emptyStateView: some View {
        VStack(spacing: 12) {
            Image(systemName: "wifi.slash")
                .font(.largeTitle)
                .foregroundColor(.secondary)
            Text("No devices nearby")
                .font(.subheadline)
            Text("Other ClipRelay devices on your network will appear here.")
                .font(.caption)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// ── Device Card ───────────────────────────────────────────────────────────────

struct DeviceCardView: View {
    let peer: PeerViewModel
    @EnvironmentObject var store: ClipRelayStore

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                // Platform icon
                Image(systemName: platformIcon(peer.platform))
                    .font(.title3)
                    .foregroundColor(.accentColor)

                VStack(alignment: .leading, spacing: 2) {
                    // Human-readable device name — NOT the UUID
                    Text(peer.displayName)
                        .font(.subheadline)
                        .fontWeight(.medium)
                    HStack(spacing: 6) {
                        connectionBadge
                        syncBadge
                    }
                }

                Spacer()

                // Ellipsis menu for less-used actions
                Menu {
                    Button("Rename…") {
                        store.beginRename(peer)
                    }
                    Divider()
                    Button("Auto-connect: \(peer.autoConnect ? "On" : "Off")") {
                        store.toggleAutoConnect(peer)
                    }
                    Divider()
                    Button("Forget Device", role: .destructive) {
                        store.forgetDevice(peer)
                    }
                    Button("Revoke Trust", role: .destructive) {
                        store.revokeTrust(peer)
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                        .foregroundColor(.secondary)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
            }

            // Primary action buttons
            HStack(spacing: 8) {
                if peer.syncEnabled {
                    Button("Pause Sync") { store.pauseSync(peer) }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                } else {
                    Button("Resume Sync") { store.resumeSync(peer) }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                }

                if peer.connected {
                    Button("Disconnect") { store.disconnect(peer) }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                }
            }
        }
        .padding(12)
        .background(Color(NSColor.controlBackgroundColor))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.primary.opacity(0.08), lineWidth: 1)
        )
    }

    private var connectionBadge: some View {
        HStack(spacing: 3) {
            Circle()
                .fill(peer.connected ? Color.green : Color.secondary.opacity(0.5))
                .frame(width: 6, height: 6)
            Text(peer.connected ? "Connected" : "Disconnected")
                .font(.caption2)
                .foregroundColor(.secondary)
        }
    }

    private var syncBadge: some View {
        Group {
            if peer.connected && !peer.syncEnabled {
                HStack(spacing: 3) {
                    Image(systemName: "pause.fill")
                        .font(.caption2)
                        .foregroundColor(.orange)
                    Text("Sync paused")
                        .font(.caption2)
                        .foregroundColor(.orange)
                }
            }
        }
    }

    private func platformIcon(_ platform: String?) -> String {
        switch platform?.lowercased() {
        case "android": return "iphone"
        case "macos": return "laptopcomputer"
        case "windows": return "pc"
        case "linux": return "server.rack"
        default: return "desktopcomputer"
        }
    }
}
