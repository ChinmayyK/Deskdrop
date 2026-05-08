#!/bin/bash
# Script to rename all ProxiBoard occurrences to ClipRelay in text files

find . -type f -not -path '*/.*' -not -path './target/*' | while read -r file; do
    if [[ "$file" == "./scripts/rename_all.sh" ]]; then continue; fi
    # Skip binary files
    if grep -qI . "$file"; then
        echo "Processing $file..."
        # Replace ProxiBoard with ClipRelay (case-sensitive where appropriate)
        sed -i '' 's/ProxiBoard/ClipRelay/g' "$file"
        sed -i '' 's/proxiboard/cliprelay/g' "$file"
        sed -i '' 's/PROXIBOARD/CLIPRELAY/g' "$file"
    fi
done
