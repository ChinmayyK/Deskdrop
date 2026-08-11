#!/usr/bin/env python3
import subprocess
import time
import sys

ADB_PATH = "/opt/homebrew/share/android-commandlinetools/platform-tools/adb"
DEVICE = "979116c"
PKG = "com.deskdrop.debug"

def p(msg):
    print(msg, flush=True)

def run_adb(args):
    cmd = [ADB_PATH, "-s", DEVICE] + args
    res = subprocess.run(cmd, capture_output=True, text=True)
    return res.stdout.strip()

p(f"=== Deskdrop Background Service Uptime Test starting on device {DEVICE} ===")

# 1. Start MainActivity to trigger foreground service startup
run_adb(["shell", "am", "start", "-n", f"{PKG}/com.deskdrop.MainActivity"])
time.sleep(3)

# 2. Get initial PID
initial_pid = run_adb(["shell", "pidof", PKG])
p(f"Initial Process PID: {initial_pid}")
if not initial_pid:
    p("ERROR: Process not running!")
    sys.exit(1)

# 3. Confirm DeskdropService is active foreground service
svc_dump = run_adb(["shell", "dumpsys", "activity", "services", PKG])
if "DeskdropService" not in svc_dump or "isForeground=true" not in svc_dump:
    p("ERROR: DeskdropService is not running in foreground state!")
    sys.exit(1)

p("DeskdropService foreground status verified. Pressing HOME key to test background execution...")
run_adb(["shell", "input", "keyevent", "KEYCODE_HOME"])
time.sleep(2)

# 4. Monitor PID continuously for 65 seconds (sampling every 5s)
duration_sec = 65
interval = 5
samples = []
start_time = time.time()
elapsed = 0

p(f"Starting {duration_sec}-second continuous PID monitoring loop...")

while elapsed < duration_sec:
    curr_time = time.strftime("%H:%M:%S")
    pid = run_adb(["shell", "pidof", PKG])
    is_alive = (pid == initial_pid)
    samples.append((elapsed, curr_time, pid, is_alive))
    
    p(f"  [{elapsed:02.0f}s / {duration_sec}s] Time: {curr_time} | Expected PID: {initial_pid} | Current PID: {pid} | Alive: {is_alive}")
    
    if not is_alive:
        p(f"CRITICAL FAIL: Process PID changed or died at t={elapsed}s! Expected {initial_pid}, got '{pid}'")
        break
    
    time.sleep(interval)
    elapsed = time.time() - start_time

end_time = time.time()
total_uptime = end_time - start_time

p(f"\n=== Monitoring Completed ===")
p(f"Total monitored duration: {total_uptime:.2f} seconds")
p(f"Initial PID: {initial_pid}")
final_pid = run_adb(["shell", "pidof", PKG])
p(f"Final PID: {final_pid}")

# 5. Verify logcat for crashes or fatal exceptions during interval
logcat_crashes = run_adb(["shell", "logcat", "-d", "-t", "1000", "*:E"])
fatal_lines = [line for line in logcat_crashes.splitlines() if PKG in line and ("FATAL" in line or "AndroidRuntime" in line or "SIGSEGV" in line or "SIGABRT" in line)]

p("\n--- Logcat Crash Check ---")
if fatal_lines:
    p("Warnings/Errors found in logcat:")
    for line in fatal_lines[:10]:
        p(f"  {line}")
else:
    p("Zero fatal exceptions or crashes found in logcat log.")

if initial_pid == final_pid and len(fatal_lines) == 0 and total_uptime >= 60.0:
    p(f"\n✅ UPTIME VERIFICATION PASSED: DeskdropService maintained process PID {initial_pid} continuously for {total_uptime:.1f}s (>60s requirement) with ZERO crashes or restarts.")
else:
    p("\n❌ UPTIME VERIFICATION FAILED!")
