#!/usr/bin/env python3
import subprocess
import os
import sys
import time

ADB_PATH = "/opt/homebrew/share/android-commandlinetools/platform-tools/adb"
DEVICE = "979116c"
PKG = "com.deskdrop.debug"
REPO_ROOT = "/Users/chinmayk/Projects/Deskdrop"

def run_cmd(cmd, cwd=None):
    res = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=cwd or REPO_ROOT)
    return res.returncode, res.stdout, res.stderr

def run_adb(args):
    cmd = [ADB_PATH, "-s", DEVICE] + args
    res = subprocess.run(cmd, capture_output=True, text=True)
    return res.returncode, res.stdout.strip()

print("=== Starting Deskdrop P2P Payload Transfer Stability Verification ===")

results = {}

# 1. Rust Protocol & Engine Level P2P Tests (Text, File, Image exchanges + Chunking + De-duplication + Latency)
print("\n[Phase 1] Executing Rust P2P Core Payload Test Suites (Text, File, Image exchanges)...")
rust_tests = [
    ("e2e_text_one_way", "cargo test --test e2e_test e2e_text_one_way"),
    ("e2e_bidirectional", "cargo test --test e2e_test e2e_bidirectional"),
    ("e2e_file_transfer", "cargo test --test e2e_test e2e_file_transfer"),
    ("e2e_image_transfer", "cargo test --test e2e_test e2e_image_transfer"),
    ("chunked_large_text_roundtrip", "cargo test --test e2e_test chunked_large_text_roundtrip"),
    ("chunked_image_roundtrip", "cargo test --test e2e_test chunked_image_roundtrip"),
    ("sim_two_nodes_exchange_text", "cargo test --test integration_test sim_two_nodes_exchange_text"),
    ("sim_image_payload_roundtrip", "cargo test --test integration_test sim_image_payload_roundtrip"),
    ("three_device_fanout_all_receive", "cargo test --test mesh_test three_device_fanout_all_receive"),
]

phase1_pass = True
for name, cmd in rust_tests:
    code, out, err = run_cmd(cmd)
    if code == 0 and "test result: ok" in out:
        print(f"  ✅ {name}: PASS")
        results[name] = "PASS"
    else:
        print(f"  ❌ {name}: FAIL\n  Stdout: {out[:200]}\n  Stderr: {err[:200]}")
        results[name] = "FAIL"
        phase1_pass = False

# 2. Android Device DeskdropService P2P Intent Handling Test (Text, File, Image payload triggers)
print("\n[Phase 2] Executing Android Service P2P Intent Stress Test on hardware device 979116c...")
initial_pid = run_adb(["shell", "pidof", PKG])[1]
print(f"Current Device Process PID: {initial_pid}")

# 2a. P2P Text Transfer Intent Trigger
print("  • Triggering P2P Text Push Intent...")
run_adb(["shell", "am", "start-foreground-service", "-n", f"{PKG}/com.deskdrop.DeskdropService",
         "-a", "com.deskdrop.PUSH_CLIPBOARD", "--es", "extra_clipboard_text", "Test P2P Text Payload 12345"])
time.sleep(2)

# 2b. P2P File Transfer Intent Trigger
print("  • Triggering P2P File Push Intent...")
# Create dummy test file on device storage
run_adb(["shell", "echo 'Deskdrop P2P Test File Contents' > /sdcard/Download/test_payload.txt"])
run_adb(["shell", "am", "start-foreground-service", "-n", f"{PKG}/com.deskdrop.DeskdropService",
         "-a", "com.deskdrop.PUSH_SHARED_URI", "--es", "extra_shared_uri", "file:///sdcard/Download/test_payload.txt", "--es", "extra_shared_name", "test_payload.txt"])
time.sleep(2)

# 2c. P2P Image Transfer Intent Trigger
print("  • Triggering P2P Image Push Intent...")
run_adb(["shell", "am", "start-foreground-service", "-n", f"{PKG}/com.deskdrop.DeskdropService",
         "-a", "com.deskdrop.PUSH_SHARED_URI", "--es", "extra_shared_uri", "file:///sdcard/Download/test_image.png", "--es", "extra_shared_name", "test_image.png"])
time.sleep(2)

# Verify process remained stable and responsive
current_pid = run_adb(["shell", "pidof", PKG])[1]
print(f"Post-Intent Process PID: {current_pid}")

phase2_pass = (initial_pid == current_pid and current_pid != "")
if phase2_pass:
    print("  ✅ Android P2P Service Intent Stress Test: PASS (No crashes, PID stable)")
    results["android_p2p_service_intent_test"] = "PASS"
else:
    print(f"  ❌ Android P2P Service Intent Stress Test: FAIL (PID changed from {initial_pid} to {current_pid})")
    results["android_p2p_service_intent_test"] = "FAIL"

# 3. Final Logcat Error Audit
code, logcat_err = run_adb(["shell", "logcat", "-d", "-t", "500", "*:E"])
fatal_lines = [l for l in logcat_err.splitlines() if PKG in l and ("FATAL" in l or "AndroidRuntime" in l or "SIGSEGV" in l or "SIGABRT" in l)]

if fatal_lines:
    print("\n❌ Logcat check found fatal errors:")
    for l in fatal_lines:
        print("  ", l)
    results["logcat_fatal_check"] = "FAIL"
else:
    print("\n  ✅ Logcat Audit: 0 fatal exceptions or crashes during P2P payload operations.")
    results["logcat_fatal_check"] = "PASS"

print("\n=== P2P Payload Transfer Stability Summary ===")
all_pass = all(v == "PASS" for v in results.values())
for k, v in results.items():
    print(f"  {k}: {v}")

if all_pass:
    print("\n✅ OVERALL P2P PAYLOAD TRANSFER STABILITY: PASSED")
else:
    print("\n❌ OVERALL P2P PAYLOAD TRANSFER STABILITY: FAILED")
