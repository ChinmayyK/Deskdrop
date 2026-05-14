// ClipboardHistoryView.swift — ClipRelay macOS v4
// Spotlight-style quick-access panel. Keyboard-first, day-grouped, live preview expand.

import SwiftUI

// MARK: - Root Panel

struct QuickAccessHistoryView: View {
    @ObservedObject var store: ClipRelayStore
    @State private var search        = ""
    @State private var selectedIndex = 0
    @State private var expandedID:   Int64? = nil          // TimelineItem.id is Int64
    @FocusState private var searchFocused: Bool

    // ── Results ───────────────────────────────────────────────────────────────

    private var results: [TimelineItem] {
        let base = store.timeline
        if search.isEmpty { return Array(base.prefix(40)) }
        return base.filter {
            $0.title.localizedCaseInsensitiveContains(search)        ||
            $0.sourceDevice.localizedCaseInsensitiveContains(search) ||
            $0.typeLabel.localizedCaseInsensitiveContains(search)    ||
            ($0.fullText?.localizedCaseInsensitiveContains(search) ?? false)
        }.prefix(40).map { $0 }
    }

    // Group by calendar day (pinned items float to top when no search)
    private var dayGroups: [(label: String, items: [TimelineItem])] {
        guard search.isEmpty else {
            return results.isEmpty ? [] : [("RESULTS", results)]
        }
        let pinned = results.filter { $0.pinned }
        let rest   = results.filter { !$0.pinned }

        var groups: [(String, [TimelineItem])] = []
        if !pinned.isEmpty { groups.append(("PINNED", pinned)) }

        let cal = Calendar.current
        let byDay = Dictionary(grouping: rest) { cal.startOfDay(for: $0.timestamp) }
        let sorted = byDay.keys.sorted(by: >)
        for day in sorted {
            let label: String
            if cal.isDateInToday(day)     { label = "TODAY" }
            else if cal.isDateInYesterday(day) { label = "YESTERDAY" }
            else {
                let f = DateFormatter(); f.dateFormat = "EEEE, MMM d"; label = f.string(from: day).uppercased()
            }
            groups.append((label, byDay[day]!))
        }
        return groups
    }

    private var flatResults: [TimelineItem] { dayGroups.flatMap { $0.items } }

    var body: some View {
        ZStack {
            // Background: HUD vibrancy + obsidian tint
            CRHUDMaterial().ignoresSafeArea()
            LinearGradient(
                stops: [
                    .init(color: Color(hex: 0x0C1025, opacity: 0.58), location: 0),
                    .init(color: Color(hex: 0x060810, opacity: 0.70), location: 1)
                ],
                startPoint: .topLeading, endPoint: .bottomTrailing
            ).ignoresSafeArea()

            VStack(spacing: 0) {
                // Search bar
                QASearchBar(text: $search, focused: $searchFocused)
                    .padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 11)

                panelSeparator

                // Just-copied strip (only when not searching)
                if let ctx = store.quickSendContext, !ctx.text.isEmpty, search.isEmpty {
                    QuickSendStrip(store: store, context: ctx)
                        .padding(.horizontal, 14).padding(.top, 11).padding(.bottom, 2)
                }

                // Content
                if flatResults.isEmpty {
                    QAEmptyState(hasSearch: !search.isEmpty)
                } else {
                    ScrollViewReader { proxy in
                        ScrollView(.vertical, showsIndicators: false) {
                            LazyVStack(spacing: 0) {
                                ForEach(Array(dayGroups.enumerated()), id: \.offset) { _, group in
                                    QAGroupLabel(text: group.label,
                                                 icon: group.label == "PINNED" ? "pin.fill" : "clock",
                                                 tint: group.label == "PINNED"
                                                     ? CRTheme.accentGold
                                                     : Color(white: 1, opacity: 0.26))
                                    ForEach(Array(group.items.enumerated()), id: \.element.id) { _, item in
                                        let globalIdx = flatResults.firstIndex(where: { $0.id == item.id }) ?? 0
                                        QuickRow(
                                            item:        item,
                                            store:       store,
                                            isSelected:  globalIdx == selectedIndex,
                                            isExpanded:  expandedID == item.id,
                                            onTap:       { store.copyTimelineItem(item) },
                                            onExpand:    {
                                                withAnimation(.crSpring) {
                                                    expandedID = expandedID == item.id ? nil : item.id
                                                }
                                            }
                                        )
                                        .id(item.id)
                                    }
                                }
                            }
                            .padding(.horizontal, 8).padding(.bottom, 8)
                        }
                        .onChange(of: selectedIndex) { idx in
                            guard idx < flatResults.count else { return }
                            withAnimation { proxy.scrollTo(flatResults[idx].id, anchor: .center) }
                        }
                    }
                }

                panelSeparator
                QAFooter()
            }
        }
        .frame(width: 500, height: 570)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .strokeBorder(Color(white: 1, opacity: 0.08), lineWidth: 0.5)
        }
        .shadow(color: .black.opacity(0.55), radius: 70, x: 0, y: 28)
        .environment(\.colorScheme, .dark)
        .onAppear { searchFocused = true }
        .onChange(of: search) { _ in selectedIndex = 0; expandedID = nil }
        // Keyboard navigation via NSEvent monitor would live in AppDelegate;
        // these buttons handle it when the window is focused.
        .background(
            Group {
                Button("") { navigate(-1) }.keyboardShortcut(.upArrow,   modifiers: [])
                Button("") { navigate(+1) }.keyboardShortcut(.downArrow, modifiers: [])
                Button("") { runSelected() }.keyboardShortcut(.return,   modifiers: [])
            }
            .frame(width: 0, height: 0).opacity(0)
        )
    }

    // MARK: Helpers

    private var panelSeparator: some View {
        Rectangle().fill(Color(white: 1, opacity: 0.07)).frame(height: 0.5)
    }

    private func navigate(_ delta: Int) {
        let max = flatResults.count - 1
        guard max >= 0 else { return }
        selectedIndex = min(max, max(0, selectedIndex + delta))
    }

    private func runSelected() {
        guard selectedIndex < flatResults.count else { return }
        store.copyTimelineItem(flatResults[selectedIndex])
    }
}

// MARK: - Search Bar

private struct QASearchBar: View {
    @Binding var text: String
    var focused: FocusState<Bool>.Binding

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(Color(white: 1, opacity: 0.28))
                .frame(width: 18)

            TextField("Search clipboard history…", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 15.5))
                .foregroundStyle(.white)
                .focused(focused)

            if !text.isEmpty {
                Button { withAnimation(.crFast) { text = "" } } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 15))
                        .foregroundStyle(Color(white: 1, opacity: 0.28))
                }
                .buttonStyle(.plain)
                .transition(.scale(scale: 0.75).combined(with: .opacity))
            } else {
                HStack(spacing: 2) { KbdChip("⌘"); KbdChip("⇧"); KbdChip("V") }
            }
        }
        .padding(.horizontal, 13).padding(.vertical, 10)
        .background {
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .fill(Color(white: 1, opacity: 0.08))
                .overlay {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .strokeBorder(Color(white: 1, opacity: 0.10), lineWidth: 0.5)
                }
        }
        .animation(.crFast, value: text.isEmpty)
    }
}

// MARK: - Group Label

private struct QAGroupLabel: View {
    let text: String; let icon: String; let tint: Color
    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: icon).font(.system(size: 9, weight: .bold)).foregroundStyle(tint)
            Text(text).font(.system(size: 9.5, weight: .bold)).tracking(1.1).foregroundStyle(tint)
            Spacer()
        }
        .padding(.horizontal, 10).padding(.top, 10).padding(.bottom, 3)
    }
}

// MARK: - Quick Send Strip

private struct QuickSendStrip: View {
    @ObservedObject var store: ClipRelayStore
    let context: QuickSendContext

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            HStack(spacing: 6) {
                // Pulsing dot
                ZStack {
                    Circle().fill(CRTheme.brandElectric.opacity(0.20)).frame(width: 14, height: 14)
                    Circle().fill(CRTheme.brandElectric).frame(width: 5.5, height: 5.5)
                }
                .crGlow(CRTheme.brandElectric, radius: 4)

                Text("JUST COPIED")
                    .font(.system(size: 9.5, weight: .bold)).tracking(1.0)
                    .foregroundStyle(CRTheme.brandElectric)
                Spacer()
                Text(context.timestamp.relativeTimeString())
                    .font(.system(size: 10.5)).foregroundStyle(Color(white: 1, opacity: 0.24))
            }

            Text(context.text)
                .font(.system(size: 12.5, weight: .medium, design: .monospaced))
                .foregroundStyle(Color(white: 1, opacity: 0.80))
                .lineLimit(2).fixedSize(horizontal: false, vertical: true)

            if !store.connectedDevices.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(store.connectedDevices) { device in
                            Button {
                                store.sendQuickContext(to: device)
                            } label: {
                                HStack(spacing: 5) {
                                    Image(systemName: "desktopcomputer").font(.system(size: 9.5))
                                    Text(device.name).font(.system(size: 11, weight: .medium))
                                }
                                .foregroundStyle(.white.opacity(0.82))
                                .padding(.horizontal, 9).padding(.vertical, 5)
                                .background {
                                    Capsule()
                                        .fill(Color(white: 1, opacity: 0.09))
                                        .overlay { Capsule().strokeBorder(Color(white: 1, opacity: 0.11), lineWidth: 0.5) }
                                }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
        .padding(12)
        .background {
            RoundedRectangle(cornerRadius: 11, style: .continuous)
                .fill(CRTheme.brandElectric.opacity(0.08))
                .overlay {
                    RoundedRectangle(cornerRadius: 11, style: .continuous)
                        .strokeBorder(CRTheme.brandElectric.opacity(0.17), lineWidth: 0.5)
                }
        }
    }
}

// MARK: - Quick Row

private struct QuickRow: View {
    let item:       TimelineItem
    @ObservedObject var store: ClipRelayStore
    var isSelected: Bool
    var isExpanded: Bool
    let onTap:      () -> Void
    let onExpand:   () -> Void

    @State private var hovered = false

    private var accent: Color {
        switch item.iconName {
        case "photo":    return CRTheme.accentPurple
        case "doc.fill": return CRTheme.accentIndigo
        case "wifi":     return CRTheme.accentGreen
        default:         return CRTheme.brandElectric
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Main row
            HStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(accent.opacity(isSelected ? 0.22 : 0.10))
                        .frame(width: 30, height: 30)
                    Image(systemName: item.iconName)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(accent).symbolRenderingMode(.hierarchical)
                }

                VStack(alignment: .leading, spacing: 2) {
                    Text(item.title)
                        .font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(.white.opacity(isSelected ? 1.0 : 0.82))
                        .lineLimit(1)
                    HStack(spacing: 4) {
                        Text(item.typeLabel).foregroundStyle(accent.opacity(0.70))
                        Text("·").foregroundStyle(Color(white: 1, opacity: 0.18)).font(.system(size: 9))
                        Text(item.sourceDevice).lineLimit(1).truncationMode(.middle)
                        Text("·").foregroundStyle(Color(white: 1, opacity: 0.18)).font(.system(size: 9))
                        Text(item.timestamp.relativeTimeString())
                    }
                    .font(.system(size: 10.5)).foregroundStyle(Color(white: 1, opacity: 0.36))
                }

                Spacer(minLength: 0)

                // Right actions — visible on select/hover
                if isSelected || hovered {
                    HStack(spacing: 5) {
                        // Expand preview (text items only)
                        if item.fullText != nil {
                            Button {
                                onExpand()
                            } label: {
                                Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 10, weight: .medium))
                                    .foregroundStyle(Color(white: 1, opacity: 0.40))
                            }
                            .buttonStyle(.plain)
                        }
                        KbdChip("↵")
                    }
                    .transition(.opacity.combined(with: .scale(scale: 0.85)))
                }

                if item.pinned && !(isSelected || hovered) {
                    Image(systemName: "pin.fill")
                        .font(.system(size: 9))
                        .foregroundStyle(CRTheme.accentGold.opacity(0.70))
                        .rotationEffect(.degrees(45))
                }
            }
            .padding(.horizontal, 10).padding(.vertical, 7)

            // Expanded preview
            if isExpanded, let preview = item.fullText, !preview.isEmpty {
                Text(preview)
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(Color(white: 1, opacity: 0.65))
                    .padding(.horizontal, 10).padding(.bottom, 9)
                    .fixedSize(horizontal: false, vertical: true)
                    .lineLimit(8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(isSelected
                      ? Color(white: 1, opacity: 0.095)
                      : (hovered ? Color(white: 1, opacity: 0.042) : .clear))
        }
        .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .onTapGesture { onTap() }
        .onHover { hovered = $0 }
        .animation(.crFast, value: isSelected)
        .animation(.crFast, value: hovered)
    }
}

// MARK: - Empty State

private struct QAEmptyState: View {
    var hasSearch: Bool
    var body: some View {
        VStack(spacing: 11) {
            Image(systemName: hasSearch ? "magnifyingglass" : "doc.on.clipboard")
                .font(.system(size: 26, weight: .ultraLight))
                .foregroundStyle(Color(white: 1, opacity: 0.16))
                .symbolRenderingMode(.hierarchical)
            Text(hasSearch ? "No results" : "Clipboard is empty")
                .font(.system(size: 13.5, weight: .medium))
                .foregroundStyle(Color(white: 1, opacity: 0.32))
            if !hasSearch {
                Text("Copy something to get started")
                    .font(.system(size: 12)).foregroundStyle(Color(white: 1, opacity: 0.18))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity).padding(.vertical, 44)
    }
}

// MARK: - Footer

private struct QAFooter: View {
    var body: some View {
        HStack(spacing: 14) {
            QAHint(keys: ["↑", "↓"], label: "navigate")
            QAHint(keys: ["↵"],       label: "copy")
            QAHint(keys: ["⌘", "↵"],  label: "send to all")
            QAHint(keys: ["Space"],    label: "preview")
            Spacer()
            QAHint(keys: ["Esc"], label: "close")
        }
        .padding(.horizontal, 16).padding(.vertical, 10)
        .background {
            Rectangle()
                .fill(Color(white: 1, opacity: 0.022))
                .overlay(alignment: .top) {
                    Rectangle().fill(Color(white: 1, opacity: 0.07)).frame(height: 0.5)
                }
        }
    }
}

private struct QAHint: View {
    let keys: [String]; let label: String
    var body: some View {
        HStack(spacing: 3) {
            ForEach(keys, id: \.self) { KbdChip($0) }
            Text(label).font(.system(size: 10.5)).foregroundStyle(Color(white: 1, opacity: 0.20))
        }
    }
}
