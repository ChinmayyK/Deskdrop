import sys

with open("deskdrop-core/src/protocol.rs", "r") as f:
    content = f.read()

# 1. Bump PROTOCOL_VERSION
content = content.replace("pub const PROTOCOL_VERSION: u16 = 5;", "pub const PROTOCOL_VERSION: u16 = 6;")

# 2. Modify Hello
old_hello = """    Hello {
        device_id: Uuid,
        device_name: String,
        identity_pubkey: [u8; 32],
        identity_proof: [u8; 32],
        metadata_json: Option<String>,
    },"""

new_hello = """    Hello {
        device_id: Uuid,
        device_name: String,
        metadata_json: Option<String>,
        fcm_token: Option<String>,
    },"""
content = content.replace(old_hello, new_hello)

# 3. Modify HelloAck
old_hello_ack = """    HelloAck {
        device_id: Uuid,
        device_name: String,
        identity_pubkey: [u8; 32],
        nonce_response: [u8; 16],
        identity_proof: [u8; 32],
        trusted: bool,
        metadata_json: Option<String>,
    },"""

new_hello_ack = """    HelloAck {
        device_id: Uuid,
        device_name: String,
        trusted: bool,
        metadata_json: Option<String>,
        fcm_token: Option<String>,
    },"""
content = content.replace(old_hello_ack, new_hello_ack)

with open("deskdrop-core/src/protocol.rs", "w") as f:
    f.write(content)
