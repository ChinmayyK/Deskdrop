#!/bin/bash
# Script to rename all ProxiBoard occurrences to Deskdrop in text files

find . -type f -not -path '*/.*' -not -path './target/*' | while read -r file; do
    if [[ "$file" == "./scripts/rename_all.sh" ]]; then continue; fi
    # Skip binary files
    if grep -qI . "$file"; then
        echo "Processing $file..."
        # Replace ProxiBoard with Deskdrop (case-sensitive where appropriate)
        sed -i '' 's/ProxiBoard/Deskdrop/g' "$file"
        sed -i '' 's/proxiboard/deskdrop/g' "$file"
        sed -i '' 's/PROXIBOARD/DESKDROP/g' "$file"
    fi
done
