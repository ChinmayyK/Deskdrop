#!/usr/bin/env python3
import os
import re
import time

def main():
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
    
    # 1. Bump Cargo.toml
    cargo_path = os.path.join(repo_root, 'deskdrop-core', 'Cargo.toml')
    with open(cargo_path, 'r') as f:
        cargo_content = f.read()
    
    # Find version = "x.y.z"
    match = re.search(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', cargo_content, re.MULTILINE)
    if match:
        major, minor, patch = match.groups()
        new_patch = int(patch) + 1
        new_version = f'{major}.{minor}.{new_patch}'
        cargo_content = cargo_content[:match.start()] + f'version = "{new_version}"' + cargo_content[match.end():]
        with open(cargo_path, 'w') as f:
            f.write(cargo_content)
        print(f"Bumped Cargo.toml to {new_version}")

    # 2. Bump build.gradle
    gradle_path = os.path.join(repo_root, 'platforms', 'android', 'app', 'build.gradle')
    with open(gradle_path, 'r') as f:
        gradle_content = f.read()
    
    # Bump versionCode
    vcode_match = re.search(r'versionCode\s+(\d+)', gradle_content)
    if vcode_match:
        new_vcode = int(vcode_match.group(1)) + 1
        gradle_content = gradle_content[:vcode_match.start(1)] + str(new_vcode) + gradle_content[vcode_match.end(1):]
    
    # Bump versionName
    vname_match = re.search(r'versionName\s+"(\d+)\.(\d+)\.(\d+)"', gradle_content)
    if vname_match:
        major, minor, patch = vname_match.groups()
        new_patch = int(patch) + 1
        new_versionName = f'{major}.{minor}.{new_patch}'
        # Using string replacement carefully
        gradle_content = re.sub(r'versionName\s+"(\d+)\.(\d+)\.(\d+)"', f'versionName "{new_versionName}"', gradle_content, count=1)
        print(f"Bumped Android versionName to {new_versionName}, versionCode to {new_vcode}")

    with open(gradle_path, 'w') as f:
        f.write(gradle_content)

    # 3. Bump Info.plist
    plist_path = os.path.join(repo_root, 'platforms', 'macos', 'Deskdrop', 'Info.plist')
    with open(plist_path, 'r') as f:
        plist_content = f.read()
    
    # Bump CFBundleShortVersionString
    short_v_pattern = r'(<key>CFBundleShortVersionString</key>\s*<string>)(\d+)\.(\d+)\.(\d+)(</string>)'
    match = re.search(short_v_pattern, plist_content)
    if match:
        major, minor, patch = match.group(2), match.group(3), match.group(4)
        new_patch = int(patch) + 1
        new_short_version = f'{major}.{minor}.{new_patch}'
        plist_content = plist_content[:match.start(2)] + new_short_version + plist_content[match.end(4):]
    
    # Bump CFBundleVersion
    v_pattern = r'(<key>CFBundleVersion</key>\s*<string>)(\d+)(</string>)'
    match2 = re.search(v_pattern, plist_content)
    if match2:
        new_v = int(match2.group(2)) + 1
        plist_content = plist_content[:match2.start(2)] + str(new_v) + plist_content[match2.end(2):]
        print(f"Bumped macOS Info.plist to {new_short_version} ({new_v})")

    with open(plist_path, 'w') as f:
        f.write(plist_content)

if __name__ == '__main__':
    main()
