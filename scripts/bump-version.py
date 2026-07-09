#!/usr/bin/env python3
import os
import re

def bump_cargo_toml(path):
    if not os.path.exists(path): return
    with open(path, 'r') as f:
        content = f.read()
    match = re.search(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', content, re.MULTILINE)
    if match:
        major, minor, patch = match.groups()
        new_version = f'{major}.{minor}.{int(patch) + 1}'
        content = content[:match.start()] + f'version = "{new_version}"' + content[match.end():]
        with open(path, 'w') as f:
            f.write(content)
        print(f"Bumped {os.path.basename(os.path.dirname(path))}/Cargo.toml to {new_version}")

def main():
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
    
    # 1. Bump Cargo.toml files
    cargo_files = [
        os.path.join(repo_root, 'deskdrop-core', 'Cargo.toml'),
        os.path.join(repo_root, 'deskdrop-cli', 'Cargo.toml'),
        os.path.join(repo_root, 'platforms', 'linux', 'Cargo.toml'),
    ]
    for c in cargo_files:
        bump_cargo_toml(c)

    # 2. Bump build.gradle
    gradle_path = os.path.join(repo_root, 'platforms', 'android', 'app', 'build.gradle')
    if os.path.exists(gradle_path):
        with open(gradle_path, 'r') as f:
            gradle_content = f.read()
        
        vcode_match = re.search(r'versionCode\s+(\d+)', gradle_content)
        if vcode_match:
            new_vcode = int(vcode_match.group(1)) + 1
            gradle_content = gradle_content[:vcode_match.start(1)] + str(new_vcode) + gradle_content[vcode_match.end(1):]
        
        vname_match = re.search(r'versionName\s+"(\d+)\.(\d+)\.(\d+)"', gradle_content)
        if vname_match:
            major, minor, patch = vname_match.groups()
            new_versionName = f'{major}.{minor}.{int(patch) + 1}'
            gradle_content = re.sub(r'versionName\s+"(\d+)\.(\d+)\.(\d+)"', f'versionName "{new_versionName}"', gradle_content, count=1)
            print(f"Bumped Android versionName to {new_versionName}, versionCode to {new_vcode}")

        with open(gradle_path, 'w') as f:
            f.write(gradle_content)

    # 3. Bump Info.plist
    plist_path = os.path.join(repo_root, 'platforms', 'macos', 'Deskdrop', 'Info.plist')
    if os.path.exists(plist_path):
        with open(plist_path, 'r') as f:
            plist_content = f.read()
        
        short_v_pattern = r'(<key>CFBundleShortVersionString</key>\s*<string>)(\d+)\.(\d+)\.(\d+)(</string>)'
        match = re.search(short_v_pattern, plist_content)
        if match:
            major, minor, patch = match.group(2), match.group(3), match.group(4)
            new_short_version = f'{major}.{minor}.{int(patch) + 1}'
            plist_content = plist_content[:match.start(2)] + new_short_version + plist_content[match.end(4):]
        
        v_pattern = r'(<key>CFBundleVersion</key>\s*<string>)(\d+)(</string>)'
        match2 = re.search(v_pattern, plist_content)
        if match2:
            new_v = int(match2.group(2)) + 1
            plist_content = plist_content[:match2.start(2)] + str(new_v) + plist_content[match2.end(2):]
            print(f"Bumped macOS Info.plist to {new_short_version} ({new_v})")

        with open(plist_path, 'w') as f:
            f.write(plist_content)

    # 4. Bump Windows .csproj
    csproj_path = os.path.join(repo_root, 'platforms', 'windows', 'Deskdrop.Windows', 'Deskdrop.Windows.csproj')
    if os.path.exists(csproj_path):
        with open(csproj_path, 'r') as f:
            csproj_content = f.read()
            
        match = re.search(r'<Version>(\d+)\.(\d+)\.(\d+)</Version>', csproj_content)
        if match:
            major, minor, patch = match.groups()
            new_version = f'{major}.{minor}.{int(patch) + 1}'
            csproj_content = csproj_content[:match.start()] + f'<Version>{new_version}</Version>' + csproj_content[match.end():]
            with open(csproj_path, 'w') as f:
                f.write(csproj_content)
            print(f"Bumped Windows csproj to {new_version}")

if __name__ == '__main__':
    main()
