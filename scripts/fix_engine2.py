import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

# Fix finalize call
content = content.replace("match transfer.finalize() {", "match transfer.finalize(sha256_checksum.clone()) {")

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)

