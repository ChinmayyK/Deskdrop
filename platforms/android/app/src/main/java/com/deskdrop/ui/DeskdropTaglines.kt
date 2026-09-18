package com.deskdrop.ui

/**
 * Hero-header taglines, shared in spirit with the macOS app (DeskdropTaglines.swift).
 * One rotating line per day, overridden by the moment's state.
 */
object DeskdropTaglines {
    private val general = listOf(
        "Your devices, perfectly aligned.",
        "Local-first connection established.",
        "Fluid continuity across your workspace.",
        "Zero-copy transfers, instant access.",
        "Your ecosystem, unified.",
        "Bridging your digital environments.",
        "Seamless native integration.",
        "High-performance local architecture.",
        "Speed without compromise.",
        "Your devices in perfect sync.",
        "Unrestricted access across screens.",
        "Encrypted local communication.",
        "Direct peer-to-peer connection.",
        "Your workspace, extended.",
        "No clouds. No limits.",
        "Instant proximity connection.",
        "The fastest path between your devices.",
        "Crafted for uninterrupted flow.",
        "A quiet, powerful connection.",
        "Native performance, everywhere.",
        "Your ecosystem, instantly responsive.",
        "Precision data orchestration.",
        "Flawless continuity enabled.",
        "The bridge is open.",
        "Bypassing the cloud entirely.",
        "Your screens, acting as one.",
        "Instantaneous local transfer.",
        "Seamless across every boundary.",
        "Your local network, optimized.",
        "Uncompromised speed and security.",
        "A seamless extension of your phone.",
        "The power of direct connection.",
        "Your files, right where you need them.",
        "Synchronized at the speed of light.",
        "Your devices, working together.",
        "Boundless local connectivity.",
        "Unleashing your local network.",
        "A unified digital experience.",
        "The fastest way to move data.",
        "Your ecosystem, perfectly balanced.",
        "Seamless interaction across devices.",
        "Local speed, native feel.",
        "Your digital life, connected.",
        "The ultimate bridge for your devices.",
        "Instant access, zero latency.",
        "Your devices, harmonized.",
        "A pure, unmediated connection.",
        "Your workflow, uninterrupted.",
        "The seamless transfer standard.",
        "Your local environment, perfected."
    )

    const val NO_DEVICE = "Waiting for your devices."
    const val TRANSFERRING = "Moving files…"
    const val MULTIPLE_DEVICES = "More devices. More possibilities."

    /**
     * Same line all day, a different one tomorrow. Stepping by 17 (coprime
     * with the pool size) walks the whole pool before any line repeats, so
     * no persisted history is needed.
     */
    fun daily(epochDay: Long = java.time.LocalDate.now().toEpochDay()): String =
        general[Math.floorMod(epochDay * 17, general.size.toLong()).toInt()]

    fun current(connectedCount: Int, isTransferring: Boolean): String = when {
        connectedCount == 0 -> NO_DEVICE
        isTransferring -> TRANSFERRING
        connectedCount > 1 -> MULTIPLE_DEVICES
        else -> daily()
    }
}
