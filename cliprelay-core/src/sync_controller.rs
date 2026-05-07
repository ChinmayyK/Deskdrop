use crate::dedup::{hash_content, ContentHash};
use crate::filter::{FilterChain, Verdict};
use crate::protocol::ClipboardContent;
use crate::settings::Settings;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum SyncDecision {
    Allow { hash: ContentHash },
    Skip { reason: String },
}

pub struct SyncController {
    filter: FilterChain,
    duplicate_window: Duration,
    debounce_window: Duration,
    recent_hashes: HashMap<ContentHash, Instant>,
    last_hash: Option<ContentHash>,
    last_copy_at: Option<Instant>,
}

impl SyncController {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            filter: FilterChain::from_settings(settings),
            duplicate_window: Duration::from_millis(settings.smart_sync_duplicate_window_ms),
            debounce_window: Duration::from_millis(settings.smart_sync_debounce_ms),
            recent_hashes: HashMap::new(),
            last_hash: None,
            last_copy_at: None,
        }
    }

    pub fn evaluate_outgoing(&mut self, content: &ClipboardContent) -> SyncDecision {
        match self.filter.run(content) {
            Verdict::Allow => {}
            Verdict::Deny { reason } => return SyncDecision::Skip { reason },
        }

        let now = Instant::now();
        self.recent_hashes
            .retain(|_, seen_at| now.duration_since(*seen_at) <= self.duplicate_window);

        let hash = hash_content(content);
        if let Some(last_seen) = self.recent_hashes.get(&hash) {
            let elapsed = now.duration_since(*last_seen);
            if elapsed <= self.duplicate_window {
                return SyncDecision::Skip {
                    reason: format!(
                        "rapid duplicate copy suppressed ({}ms window)",
                        self.duplicate_window.as_millis()
                    ),
                };
            }
        }

        if self.last_hash == Some(hash) {
            if let Some(last_copy_at) = self.last_copy_at {
                if now.duration_since(last_copy_at) <= self.debounce_window {
                    return SyncDecision::Skip {
                        reason: format!(
                            "copy event debounced ({}ms window)",
                            self.debounce_window.as_millis()
                        ),
                    };
                }
            }
        }

        self.recent_hashes.insert(hash, now);
        self.last_hash = Some(hash);
        self.last_copy_at = Some(now);
        SyncDecision::Allow { hash }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            smart_sync_duplicate_window_ms: 1_000,
            smart_sync_debounce_ms: 100,
            ..Settings::default()
        }
    }

    #[test]
    fn sensitive_text_can_be_blocked() {
        let mut settings = settings();
        settings.block_sensitive_text = true;
        let mut controller = SyncController::from_settings(&settings);
        let decision =
            controller.evaluate_outgoing(&ClipboardContent::Text("password: hunter2".into()));
        assert!(matches!(decision, SyncDecision::Skip { .. }));
    }

    #[test]
    fn duplicate_copy_is_suppressed() {
        let mut controller = SyncController::from_settings(&settings());
        let content = ClipboardContent::Text("hello".into());
        assert!(matches!(
            controller.evaluate_outgoing(&content),
            SyncDecision::Allow { .. }
        ));
        assert!(matches!(
            controller.evaluate_outgoing(&content),
            SyncDecision::Skip { .. }
        ));
    }
}
