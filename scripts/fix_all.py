import re
import sys

with open("deskdrop-core/src/engine.rs", "r") as f:
    content = f.read()

content = content.replace("crate::engine::SharedState", "SharedState")
content = content.replace("crate::protocol::FileTransferMessage::Chunk", "crate::file_transfer::FileTransferMessage::Chunk")

# Fix all_sent blocks
# It looks like:
# let all_sent = {
#     let mut mgr = bg_shared.file_transfers.lock().await;
#     mgr.get_outbound_mut(&bg_transfer_id)
#         .map(|transfer| transfer.is_all_sent())
#         .unwrap_or(false)
# };
# if all_sent {
#     let _ = bg_outbox
#         .send(AppMessage::FileTransferComplete {
#             transfer_id: bg_transfer_id,
#         })
#         .await;
# }

all_sent_regex = r"let all_sent = \{\s*let mut mgr = bg_shared\.file_transfers\.lock\(\)\.await;\s*mgr\.get_outbound_mut\(&bg_transfer_id\)\s*\.map\(\|transfer\| transfer\.is_all_sent\(\)\)\s*\.unwrap_or\(false\)\s*\};\s*if all_sent \{\s*let _ = bg_outbox\s*\.send\(AppMessage::FileTransferComplete \{\s*transfer_id: bg_transfer_id,\s*\}\)\s*\.await;\s*\}"

new_all_sent = """let final_checksum = {
    let mut mgr = bg_shared.file_transfers.lock().await;
    mgr.get_outbound_mut(&bg_transfer_id)
        .and_then(|transfer| if transfer.is_all_sent() { Some(transfer.finalize_checksum()) } else { None })
};
if let Some(sha256_checksum) = final_checksum {
    let _ = bg_outbox
        .send(AppMessage::FileTransferComplete {
            transfer_id: bg_transfer_id,
            sha256_checksum,
        })
        .await;
}"""

# Fix indentation dynamically
def replace_all_sent(match):
    lines = match.group(0).split('\n')
    indent = len(lines[0]) - len(lines[0].lstrip())
    new_lines = new_all_sent.split('\n')
    res = []
    for i, line in enumerate(new_lines):
        if i == 0:
            res.append(" " * indent + line)
        else:
            # new_all_sent is indented with 4 spaces for the block
            res.append(" " * indent + line)
    return '\n'.join(res)

content = re.sub(all_sent_regex, replace_all_sent, content)

# Replace the remaining old loops!
# They look like:
# let (batch, progresses): (Vec<AppMessage>, Vec<(crate::file_transfer::TransferProgress, String)>) = { ... };
# or
# let batch: Vec<AppMessage> = { ... };
# I will just use regex to replace `let .* = \{\s*let mut mgr = bg_shared\.file_transfers\.lock\(\)\.await;.*?match transfer\.next_chunk_message\(\) \{.*?\};\s*`
# Actually, let's just find next_chunk_message
loop_regex = r"let (?:batch|\(batch,\s*progresses\)).*?=\{\s*let mut mgr = bg_shared\.file_transfers\.lock\(\)\.await;.*?match transfer\.next_chunk_message\(\).*?msgs\n\s*\}\s*;\n\s*(?:for \(prog, file_name\) in progresses \{.*?\n\s*\})?"
# That's too complex. Let's do it carefully.

with open("deskdrop-core/src/engine.rs", "w") as f:
    f.write(content)
