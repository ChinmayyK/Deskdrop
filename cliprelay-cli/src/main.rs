//! cliprelay-cli — live daemon control + product command surface.

use anyhow::{bail, Context, Result};
use cliprelay_core::{
    history::{History, HistoryPayload},
    ipc::{IpcRequest, IpcResponse},
    settings::{
        default_history_path, default_settings_path, default_trust_store_path, SettingsStore,
    },
    trust::{format_fingerprint, TrustStore},
};
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("\x1b[31mError:\x1b[0m {:#}", error);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = normalize_args(std::env::args().skip(1).collect());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    match refs.as_slice() {
        [] | ["status"] => cmd_status().await,
        ["ping"] => cmd_ping().await,
        ["version"] | ["--version"] | ["-V"] => {
            println!("cliprelay-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ["push", text] => cmd_push(text).await,
        ["send", target, text] => cmd_send(target, text).await,
        ["connect", ip] => cmd_connect(ip, cliprelay_core::protocol::DEFAULT_PORT).await,
        ["connect", ip, port] => cmd_connect(ip, port.parse().context("bad port")?).await,
        ["peers"] => cmd_peers().await,
        ["events"] => cmd_events(20).await,
        ["events", "--last", n] => cmd_events(n.parse().context("bad N")?).await,
        ["devices"] | ["devices", "list"] => cmd_devices_list(),
        ["devices", "show", id] => cmd_devices_show(id).await,
        ["devices", "trust", id] | ["devices", "retrust", id] => cmd_devices_trust(id).await,
        ["devices", "reject", id] => cmd_devices_reject(id).await,
        ["devices", "revoke", id] => cmd_devices_revoke(id),
        ["devices", "rename", id, name] => cmd_devices_rename(id, name).await,
        ["history"] => cmd_history(20, None).await,
        ["history", "--last", n] => cmd_history(n.parse().context("bad N")?, None).await,
        ["history", "--search", q] => cmd_history(100, Some(*q)).await,
        ["history", "--last", n, "--search", q] => {
            cmd_history(n.parse().context("bad N")?, Some(*q)).await
        }
        ["history", "clear"] => cmd_history_clear().await,
        ["history", "pin", id] => cmd_history_pin(id, true).await,
        ["history", "unpin", id] => cmd_history_pin(id, false).await,
        ["history", "repush", id] => cmd_history_repush(id, None).await,
        ["history", "repush", id, target] => cmd_history_repush(id, Some(target)).await,
        ["history", "delete", id] => cmd_history_delete(id).await,
        ["history", "export"] | ["history", "export", "csv"] => cmd_history_export_csv().await,
        ["metrics"] => cmd_metrics().await,
        ["settings"] | ["settings", "get"] => cmd_settings_get(None),
        ["settings", "get", key] => cmd_settings_get(Some(key)),
        ["settings", "set", key, value] => cmd_settings_set(key, value),
        ["settings", "reset"] => cmd_settings_reset(),
        ["sync", "on"] => cmd_sync(true).await,
        ["sync", "off"] => cmd_sync(false).await,
        ["stop"] => cmd_stop().await,
        ["help"] | ["--help"] | ["-h"] => {
            print_help();
            Ok(())
        }
        other => bail!(
            "Unknown command: '{}'\n\nRun `cliprelay-cli help` to see all available commands.",
            other.join(" ")
        ),
    }
}

fn normalize_args(mut args: Vec<String>) -> Vec<String> {
    if let Some(first) = args.first_mut() {
        if let Some(stripped) = first.strip_prefix('/') {
            *first = stripped.to_string();
        }
    }
    args
}

async fn try_ipc(req: &IpcRequest) -> Option<IpcResponse> {
    #[cfg(unix)]
    {
        use cliprelay_core::ipc::client::IpcClient;
        let mut client = IpcClient::connect().await.ok()?;
        client.request(req).await.ok()
    }
    #[cfg(not(unix))]
    {
        let _ = req;
        None
    }
}

async fn ipc(req: &IpcRequest) -> Result<IpcResponse> {
    try_ipc(req)
        .await
        .context("daemon not running — start with `cliprelay-daemon`")
}

async fn cmd_status() -> Result<()> {
    if let Some(IpcResponse::Ok { data: Some(data) }) = try_ipc(&IpcRequest::Status).await {
        let uptime = data["uptime_secs"].as_u64().unwrap_or(0);
        println!("ClipRelay — Live Status\n{}", "═".repeat(40));
        println!(
            "  Device:   {}",
            data["device_name"].as_str().unwrap_or("?")
        );
        println!("  Port:     {}", data["port"]);
        println!(
            "  Sync:     {}",
            bool_icon(data["sync_enabled"].as_bool().unwrap_or(true))
        );
        println!("  Peers:    {}", data["peer_count"]);
        println!(
            "  Sent:     {} pushes / {} KB",
            data["pushes_sent"],
            data["bytes_sent"].as_u64().unwrap_or(0) / 1024
        );
        println!(
            "  Received: {} pushes / {} KB",
            data["pushes_received"],
            data["bytes_received"].as_u64().unwrap_or(0) / 1024
        );
        println!("  Uptime:   {}", fmt_dur(uptime));
        return Ok(());
    }

    let trust = TrustStore::load(default_trust_store_path())?;
    let settings = SettingsStore::load(default_settings_path())?;
    let history = History::load(default_history_path())?;
    println!("ClipRelay — Offline State\n{}", "═".repeat(40));
    println!("  Daemon:   ⚠️  not running");
    println!("  Device:   {}", settings.get().resolved_device_name());
    println!("  Port:     {}", settings.get().port);
    println!("  Trusted:  {} device(s)", trust.device_count());
    println!("  History:  {} items", history.entries().len());
    Ok(())
}

async fn cmd_ping() -> Result<()> {
    let started = std::time::Instant::now();
    match ipc(&IpcRequest::Ping).await? {
        IpcResponse::Pong { uptime_secs } => println!(
            "✅  pong — {}ms RTT — uptime {}",
            started.elapsed().as_millis(),
            fmt_dur(uptime_secs)
        ),
        response => bail!("{:?}", response),
    }
    Ok(())
}

async fn cmd_push(text: &str) -> Result<()> {
    print_dispatch_response(
        ipc(&IpcRequest::PushText {
            text: text.to_string(),
        })
        .await?,
    )
}

async fn cmd_send(target: &str, text: &str) -> Result<()> {
    print_dispatch_response(
        ipc(&IpcRequest::PushTextTo {
            text: text.to_string(),
            target: target.to_string(),
        })
        .await?,
    )
}

fn print_dispatch_response(response: IpcResponse) -> Result<()> {
    match response {
        IpcResponse::Ok { data: Some(data) } => {
            let peers = data["peers"].as_array().cloned().unwrap_or_default();
            let delivered = peers
                .iter()
                .filter(|peer| {
                    peer["delivered"].as_bool().unwrap_or(false)
                        && !peer["metadata_only"].as_bool().unwrap_or(false)
                })
                .count();
            println!("✅  queued clipboard to {} peer(s)", delivered);
            for peer in peers {
                let name = peer["device_name"].as_str().unwrap_or("?");
                let delivered = peer["delivered"].as_bool().unwrap_or(false);
                let metadata_only = peer["metadata_only"].as_bool().unwrap_or(false);
                let reason = peer["reason"].as_str().unwrap_or("");
                if delivered && !metadata_only {
                    println!("  • {}", name);
                } else if delivered {
                    println!("  · metadata only → {}", name);
                } else if !reason.is_empty() {
                    println!("  × {} ({})", name, reason);
                }
            }
            Ok(())
        }
        IpcResponse::Ok { data: None } => Ok(()),
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
}

async fn cmd_connect(ip: &str, port: u16) -> Result<()> {
    match ipc(&IpcRequest::ConnectPeer {
        ip: ip.to_string(),
        port,
    })
    .await?
    {
        IpcResponse::Ok { .. } => println!("✅  connect attempt to {}:{} started", ip, port),
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
    Ok(())
}

async fn cmd_peers() -> Result<()> {
    match ipc(&IpcRequest::Peers).await? {
        IpcResponse::Ok { data: Some(data) } => {
            let peers = data.as_array().cloned().unwrap_or_default();
            if peers.is_empty() {
                println!("No peers connected.");
                return Ok(());
            }
            println!(
                "{:<36}  {:<20}  {:<15}  {:<10}  Last sync",
                "Device ID", "Name", "Endpoint", "State"
            );
            println!("{}", "─".repeat(100));
            for peer in &peers {
                let endpoint = format!(
                    "{}:{}",
                    peer["ip"].as_str().unwrap_or("?"),
                    peer["port"].as_u64().unwrap_or(0)
                );
                println!(
                    "{:<36}  {:<20}  {:<15}  {:<10}  {}",
                    peer["id"].as_str().unwrap_or("?"),
                    trunc(peer["friendly_name"].as_str().unwrap_or("?"), 20),
                    trunc(&endpoint, 15),
                    peer["status"].as_str().unwrap_or("?"),
                    peer["last_sync"]
                        .as_u64()
                        .map(fmt_ts)
                        .unwrap_or_else(|| "—".into())
                );
            }
            println!("\n{} peer(s) connected.", peers.len());
        }
        _ => println!("No peer data (daemon not running?)."),
    }
    Ok(())
}

async fn cmd_events(last: usize) -> Result<()> {
    match ipc(&IpcRequest::Feedback { last }).await? {
        IpcResponse::Ok { data: Some(data) } => {
            let events = data.as_array().cloned().unwrap_or_default();
            if events.is_empty() {
                println!("No feedback events yet.");
                return Ok(());
            }
            println!("{:<17}  {:<22}  Message", "Time", "Kind");
            println!("{}", "─".repeat(90));
            for event in &events {
                println!(
                    "{:<17}  {:<22}  {}",
                    fmt_ts(event["timestamp"].as_u64().unwrap_or(0)),
                    event["kind"].as_str().unwrap_or("?"),
                    event["message"].as_str().unwrap_or("?")
                );
            }
            println!("\n{} event(s).", events.len());
        }
        IpcResponse::Error { message } => bail!("{}", message),
        _ => println!("No feedback events (daemon not running?)."),
    }
    Ok(())
}

fn cmd_devices_list() -> Result<()> {
    let trust = TrustStore::load(default_trust_store_path())?;
    let devices: Vec<_> = trust.all_devices().collect();
    if devices.is_empty() {
        println!("No trusted devices.");
        return Ok(());
    }

    println!(
        "{:<36}  {:<18}  {:<10}  {:<17}  Last seen",
        "UUID", "Name", "State", "Fingerprint"
    );
    println!("{}", "─".repeat(104));
    for device in &devices {
        let fingerprint = format_fingerprint(&device.key_fingerprint);
        println!(
            "{:<36}  {:<18}  {:<10}  {}  {}",
            device.device_id,
            trunc(device.effective_name(), 18),
            format!("{:?}", device.state).to_lowercase(),
            &fingerprint[..17],
            fmt_ts(device.last_seen)
        );
    }
    println!("\n{} device(s).", devices.len());
    Ok(())
}

async fn cmd_devices_show(id_str: &str) -> Result<()> {
    let id = Uuid::parse_str(id_str).context("invalid UUID")?;
    if let Some(IpcResponse::Ok { data: Some(data) }) = try_ipc(&IpcRequest::DeviceDetails {
        device_id: id.to_string(),
    })
    .await
    {
        println!("Device: {}", data["effective_name"].as_str().unwrap_or("?"));
        println!(
            "  UUID:          {}",
            data["device_id"].as_str().unwrap_or("?")
        );
        println!(
            "  Current name:  {}",
            data["device_name"].as_str().unwrap_or("?")
        );
        if let Some(alias) = data["display_name"].as_str() {
            println!("  Alias:         {}", alias);
        }
        println!("  State:         {}", data["state"].as_str().unwrap_or("?"));
        println!(
            "  Fingerprint:   {}",
            data["fingerprint"].as_str().unwrap_or("?")
        );
        println!(
            "  First seen:    {}",
            fmt_ts(data["first_seen"].as_u64().unwrap_or(0))
        );
        println!(
            "  Trusted since: {}",
            data["trusted_since"]
                .as_u64()
                .map(fmt_ts)
                .unwrap_or_else(|| "—".into())
        );
        println!(
            "  Last seen:     {}",
            fmt_ts(data["last_seen"].as_u64().unwrap_or(0))
        );
        return Ok(());
    }

    let trust = TrustStore::load(default_trust_store_path())?;
    let record = trust.get(id).context("device not found")?;
    println!("Device: {}", record.effective_name());
    println!("  UUID:          {}", record.device_id);
    println!("  Current name:  {}", record.device_name);
    if let Some(alias) = &record.display_name {
        println!("  Alias:         {}", alias);
    }
    println!("  State:         {:?}", record.state);
    println!(
        "  Fingerprint:   {}",
        format_fingerprint(&record.key_fingerprint)
    );
    println!("  First seen:    {}", fmt_ts(record.first_seen));
    println!(
        "  Trusted since: {}",
        record
            .trusted_since
            .map(fmt_ts)
            .unwrap_or_else(|| "—".into())
    );
    println!("  Last seen:     {}", fmt_ts(record.last_seen));
    Ok(())
}

async fn cmd_devices_trust(id_str: &str) -> Result<()> {
    let id = Uuid::parse_str(id_str).context("invalid UUID")?;
    match ipc(&IpcRequest::TrustPeer {
        device_id: id.to_string(),
    })
    .await?
    {
        IpcResponse::Ok { .. } => println!("✅  trusted {}", id),
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
    Ok(())
}

async fn cmd_devices_reject(id_str: &str) -> Result<()> {
    let id = Uuid::parse_str(id_str).context("invalid UUID")?;
    match ipc(&IpcRequest::RejectPeer {
        device_id: id.to_string(),
    })
    .await?
    {
        IpcResponse::Ok { .. } => println!("✅  rejected {}", id),
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
    Ok(())
}

fn cmd_devices_revoke(id_str: &str) -> Result<()> {
    let id = Uuid::parse_str(id_str).context("invalid UUID")?;
    let mut trust = TrustStore::load(default_trust_store_path())?;
    if trust.revoke(id)? {
        println!("✅  revoked {}", id);
    } else {
        println!("⚠️  device {} not found", id);
    }
    Ok(())
}

async fn cmd_devices_rename(id_str: &str, name: &str) -> Result<()> {
    let id = Uuid::parse_str(id_str).context("invalid UUID")?;
    match ipc(&IpcRequest::RenameTrustedDevice {
        device_id: id.to_string(),
        display_name: name.to_string(),
    })
    .await?
    {
        IpcResponse::Ok { .. } => println!("✅  {} renamed to '{}'", id, name),
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
    Ok(())
}

async fn cmd_history(last: usize, search: Option<&str>) -> Result<()> {
    let response = if let Some(query) = search {
        try_ipc(&IpcRequest::HistorySearch {
            query: query.to_string(),
            limit: last,
        })
        .await
    } else {
        try_ipc(&IpcRequest::History { last }).await
    };

    let entries: Vec<serde_json::Value> = match response {
        Some(IpcResponse::Ok { data: Some(data) }) => data.as_array().cloned().unwrap_or_default(),
        _ => {
            let history = History::load(default_history_path())?;
            if let Some(query) = search {
                history
                    .search(query)
                    .take(last)
                    .map(|entry| serde_json::to_value(entry).unwrap())
                    .collect()
            } else {
                history
                    .recent(last)
                    .map(|entry| serde_json::to_value(entry).unwrap())
                    .collect()
            }
        }
    };

    if entries.is_empty() {
        println!("No clipboard history.");
        return Ok(());
    }

    println!(
        "{:<6}  {:<17}  {:<18}  {:<6}  Summary",
        "#", "Time", "Source", "Pin"
    );
    println!("{}", "─".repeat(96));
    for entry in &entries {
        println!(
            "{:<6}  {:<17}  {:<18}  {:<6}  {}",
            entry["id"].as_u64().unwrap_or(0),
            fmt_ts(entry["timestamp"].as_u64().unwrap_or(0)),
            trunc(entry["source_device"].as_str().unwrap_or("?"), 18),
            if entry["pinned"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                ""
            },
            payload_summary(entry.get("payload"))
        );
    }
    println!("\n{} entries.", entries.len());
    Ok(())
}

async fn cmd_history_pin(id_str: &str, pinned: bool) -> Result<()> {
    let id = id_str.parse::<u64>().context("invalid history id")?;
    match ipc(&IpcRequest::HistoryPin { id, pinned }).await? {
        IpcResponse::Ok { .. } => println!(
            "✅  history item {} {}",
            id,
            if pinned { "pinned" } else { "unpinned" }
        ),
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
    Ok(())
}

async fn cmd_history_repush(id_str: &str, target: Option<&str>) -> Result<()> {
    let id = id_str.parse::<u64>().context("invalid history id")?;
    print_dispatch_response(
        ipc(&IpcRequest::HistoryRepush {
            id,
            target: target.map(str::to_owned),
        })
        .await?,
    )
}

async fn cmd_history_clear() -> Result<()> {
    if let Some(IpcResponse::Ok { .. }) = try_ipc(&IpcRequest::HistoryClear).await {
        println!("✅  history cleared (live)");
        return Ok(());
    }
    let mut history = History::load(default_history_path())?;
    let count = history.entries().len();
    history.clear()?;
    println!("✅  cleared {} entries", count);
    Ok(())
}

async fn cmd_history_delete(id_str: &str) -> Result<()> {
    let id: u64 = id_str.parse().context("id must be a number")?;
    // Try live daemon first; fall back to direct file edit.
    if let Some(IpcResponse::Ok { .. }) = try_ipc(&IpcRequest::HistoryDelete { id }).await {
        println!("✅  entry {} deleted (live)", id);
        return Ok(());
    }
    let mut history = History::load(default_history_path())?;
    if history.remove(id)? {
        println!("✅  entry {} deleted", id);
    } else {
        bail!("entry {} not found in history", id);
    }
    Ok(())
}

async fn cmd_history_export_csv() -> Result<()> {
    // Try live daemon first.
    if let Some(IpcResponse::Ok { data: Some(data) }) =
        try_ipc(&IpcRequest::HistoryExportCsv).await
    {
        if let Some(csv) = data.as_str() {
            print!("{}", csv);
            return Ok(());
        }
    }
    // Fallback: read from disk.
    let history = History::load(default_history_path())?;
    print!("{}", history.export_csv());
    Ok(())
}

async fn cmd_metrics() -> Result<()> {
    match ipc(&IpcRequest::GetMetrics).await? {
        IpcResponse::Ok { data: Some(data) } => {
            if let Ok(pretty) = serde_json::to_string_pretty(&data) {
                println!("{}", pretty);
            } else {
                println!("{}", data);
            }
        }
        IpcResponse::Error { message } => bail!("{}", message),
        _ => bail!("daemon did not respond with metrics"),
    }
    Ok(())
}

fn cmd_settings_get(key: Option<&str>) -> Result<()> {
    let store = SettingsStore::load(default_settings_path())?;
    let value = serde_json::to_value(store.get())?;
    if let Some(key) = key {
        println!(
            "{} = {}",
            key,
            value.get(key).context(format!("unknown key '{}'", key))?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(store.get())?);
    }
    Ok(())
}

fn cmd_settings_set(key: &str, value: &str) -> Result<()> {
    let mut store = SettingsStore::load(default_settings_path())?;
    let parsed: serde_json::Value =
        serde_json::from_str(value).unwrap_or(serde_json::Value::String(value.to_string()));
    store.patch(&serde_json::json!({ key: parsed }).to_string())?;
    println!("✅  {} = {}", key, parsed);
    Ok(())
}

fn cmd_settings_reset() -> Result<()> {
    SettingsStore::load(default_settings_path())?.reset()?;
    println!("✅  settings reset to defaults");
    Ok(())
}

async fn cmd_sync(enabled: bool) -> Result<()> {
    match ipc(&IpcRequest::SetSyncEnabled { enabled }).await? {
        IpcResponse::Ok { .. } => {
            println!("✅  sync {}", if enabled { "enabled" } else { "disabled" })
        }
        IpcResponse::Error { message } => bail!("{}", message),
        response => bail!("{:?}", response),
    }
    Ok(())
}

async fn cmd_stop() -> Result<()> {
    try_ipc(&IpcRequest::Shutdown).await;
    println!("✅  daemon stopped");
    Ok(())
}

fn bool_icon(value: bool) -> &'static str {
    if value {
        "✅ on"
    } else {
        "❌ off"
    }
}

fn fmt_dur(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else if secs < 86_400 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d{}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

fn fmt_ts(unix: u64) -> String {
    let elapsed = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH + Duration::from_secs(unix))
        .unwrap_or_default()
        .as_secs();
    if elapsed < 60 {
        format!("{}s ago", elapsed)
    } else if elapsed < 3600 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{}h ago", elapsed / 3600)
    } else {
        format!("{}d ago", elapsed / 86_400)
    }
}

fn trunc(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn payload_summary(payload: Option<&serde_json::Value>) -> String {
    let Some(payload) = payload else {
        return "—".into();
    };
    let payload_type = payload["type"].as_str().unwrap_or("");
    match payload_type {
        "Text" => payload["preview"]
            .as_str()
            .map(|text| trunc(text.lines().next().unwrap_or("").trim(), 46).to_string())
            .unwrap_or_else(|| "—".into()),
        "Image" => format!(
            "[Image {} {}KB]",
            payload["mime"].as_str().unwrap_or("?"),
            payload["bytes"].as_u64().unwrap_or(0) / 1024
        ),
        "File" => format!(
            "[File '{}' {}KB]",
            trunc(payload["name"].as_str().unwrap_or("?"), 20),
            payload["bytes"].as_u64().unwrap_or(0) / 1024
        ),
        "Metadata" => payload["summary"].as_str().unwrap_or("—").to_string(),
        _ => {
            if let Ok(parsed) = serde_json::from_value::<HistoryPayload>(payload.clone()) {
                match parsed {
                    HistoryPayload::Text { preview, .. } => preview,
                    HistoryPayload::Image { mime, bytes } => {
                        format!("[Image {} {}KB]", mime, bytes / 1024)
                    }
                    HistoryPayload::File { name, bytes } => {
                        format!("[File '{}' {}KB]", trunc(&name, 20), bytes / 1024)
                    }
                    HistoryPayload::Metadata { summary, .. } => summary,
                }
            } else {
                "—".into()
            }
        }
    }
}

fn print_help() {
    println!(
        r#"cliprelay-cli {}  —  ClipRelay management tool

USAGE:  cliprelay-cli <command> [args]
        cliprelay-cli /history            (leading slash prefix accepted)
        cliprelay-cli /send <device> "<text>"

DAEMON CONTROL
  status                          Show status (live from daemon or offline snapshot)
  ping                            Check daemon health / measure IPC round-trip time
  push "<text>"                   Push text to all connected peers
  send <device> "<text>"          Push text to one specific device (name or UUID prefix)
  connect <ip> [port]             Manually connect to a peer by IP address
  sync on|off                     Enable or disable clipboard syncing globally
  stop                            Gracefully stop the daemon
  version                         Print version and exit
  metrics                         Print live runtime metrics (JSON)

PEERS
  peers                           List currently connected peers with stats
  events [--last N]               Show recent feedback events (default: last 20)

DEVICES
  devices list                    List all known devices and their trust state
  devices show <uuid>             Show full trust details and key fingerprint
  devices trust <uuid>            Trust an untrusted device (TOFU accept)
  devices retrust <uuid>          Alias for 'trust'
  devices reject <uuid>           Reject a device (deny this session, don't remember)
  devices revoke <uuid>           Revoke trust for a previously-trusted device
  devices rename <uuid> <name>    Assign a human-readable display name

HISTORY
  history [--last N]              Show recent clipboard history (default: 20)
  history --search <query>        Full-text search through history
  history --last N --search <q>   Combine limit and search
  history pin <id>                Pin an entry so it survives the ring-buffer limit
  history unpin <id>              Remove a pin
  history repush <id> [device]    Re-send a stored text entry (optional: to one device)
  history delete <id>             Delete a single history entry
  history export [csv]            Export full history as CSV (stdout)
  history clear                   Clear all history (irreversible)

SETTINGS
  settings get [<key>]            Print all settings (JSON) or one key's value
  settings set <key> <value>      Update a setting; value is JSON-typed
  settings reset                  Reset all settings to factory defaults

COMMON SETTINGS KEYS
  device_name                     Override local device name (empty = use hostname)
  sync_enabled                    bool — master sync on/off switch
  sync_text / sync_images         bool — sync specific content types
  sync_files                      bool — sync file transfers
  max_payload_bytes               Maximum synced payload (bytes)
  history_limit                   Max entries retained (20–100)
  max_history_text_bytes          Max stored text per history entry
  block_sensitive_text            bool — heuristic password/secret suppression
  smart_sync_duplicate_window_ms  Suppress identical copies within this window (ms)
  smart_sync_debounce_ms          Coalesce rapid copy bursts (ms)
  timeline_first_mode             bool — remote items land in feed before clipboard
  auto_apply_remote_clipboard     bool — auto-apply from feed (requires timeline_first)
  clipboard_poll_ms               Local clipboard poll interval (min 10 ms)
  require_tofu_confirmation       bool — prompt for new device trust (recommended: true)
  start_on_login                  bool — launch daemon on system login

EXAMPLES
  cliprelay-cli /history
  cliprelay-cli history --search "github"
  cliprelay-cli send macbook "meeting at 3pm"
  cliprelay-cli history repush 42 windows-pc
  cliprelay-cli history delete 17
  cliprelay-cli devices show 550e8400-e29b-41d4-a716-446655440000
  cliprelay-cli settings set block_sensitive_text true
  cliprelay-cli events --last 10
"#,
        env!("CARGO_PKG_VERSION")
    );
}
