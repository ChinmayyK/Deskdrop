import re

def process_file(path, replacements):
    with open(path, "r") as f:
        content = f.read()
    for target, replacement in replacements:
        content = content.replace(target, replacement)
    with open(path, "w") as f:
        f.write(content)

# trust.rs
process_file("deskdrop-core/src/trust.rs", [
    ("    #[serde(default)]\n    pub fcm_token: Option<String>,\n", ""),
    ("            fcm_token: None,\n", ""),
    ("        fcm_token: Option<String>,\n", ""),
    ("                fcm_token: fcm_token.clone(),\n", ""),
    ("        if fcm_token.is_some() {\n            record.fcm_token = fcm_token;\n        }\n", ""),
    ("        let record = self.observe_peer(device_id, device_name, public_key, None)?;\n", "        let record = self.observe_peer(device_id, device_name, public_key)?;\n"),
    ("        self.observe_peer(device_id, device_name, &public_key, None)?;\n", "        self.observe_peer(device_id, device_name, &public_key)?;\n")
])

# protocol.rs
with open("deskdrop-core/src/protocol.rs", "r") as f:
    content = f.read()
content = re.sub(r"        #\[serde\(default\)\]\s*fcm_token: Option<String>,", "", content)
with open("deskdrop-core/src/protocol.rs", "w") as f:
    f.write(content)

# network.rs
process_file("deskdrop-core/src/network.rs", [
    ("    pub fcm_token: Option<String>,\n", ""),
    ("        fcm_token,\n", ""),
])

# engine.rs
process_file("deskdrop-core/src/engine.rs", [
    ("        trust.observe_peer(device_id, device_name, &public_key, None)?;\n", "        trust.observe_peer(device_id, device_name, &public_key)?;\n"),
    ("    fcm_token: Option<String>,\n", ""),
    ("        trust.observe_peer(device_id, device_name.clone(), &identity_pubkey, fcm_token)?\n", "        trust.observe_peer(device_id, device_name.clone(), &identity_pubkey)?\n"),
    ("        hs.fcm_token.clone(),\n", "")
])
