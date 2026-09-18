using System;

namespace Deskdrop.WinUI.Services
{
    /// <summary>
    /// Header taglines, shared in spirit with the macOS app (DeskdropTaglines.swift).
    /// One rotating line per day, overridden by the moment's state.
    /// </summary>
    public static class DeskdropTaglines
    {
        private static readonly string[] General =
        {
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
            "A seamless extension of your PC.",
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
        };

        public const string NoDevice = "Waiting for your devices.";
        public const string Transferring = "Moving files…";
        public const string MultipleDevices = "More devices. More possibilities.";

        // Same line all day, a different one tomorrow. Stepping by 17 (coprime
        // with the pool size) walks the whole pool before any line repeats, so
        // no persisted history is needed.
        public static string Daily()
        {
            long day = DateTime.Today.Ticks / TimeSpan.TicksPerDay;
            return General[(int)((day * 17) % General.Length)];
        }

        public static string Current(int connectedCount, bool isTransferring)
        {
            if (connectedCount == 0) return NoDevice;
            if (isTransferring) return Transferring;
            if (connectedCount > 1) return MultipleDevices;
            return Daily();
        }
    }
}
