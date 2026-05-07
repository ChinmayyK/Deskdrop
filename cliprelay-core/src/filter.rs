//! Content filter — applied before sending and on receipt.
//!
//! Filters run in order; the first `Deny` result short-circuits the chain.
//!
//! # Built-in filters
//! - `SizeFilter`       — reject payloads above the configured size limit.
//! - `TypeFilter`       — allow/deny by content type (text / image / file).
//! - `ExtensionFilter`  — block file extensions known to be dangerous.
//! - `SensitiveTextFilter` — optional heuristic to avoid syncing passwords
//!   (patterns like `password: ...`, `BEGIN RSA PRIVATE KEY`, etc.).
//!
//! # Extension
//! Platform code can push custom `Box<dyn Filter>` into the `FilterChain`.

use crate::protocol::ClipboardContent;

// ── Verdict ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Allow,
    Deny { reason: String },
}

impl Verdict {
    pub fn is_allow(&self) -> bool {
        *self == Verdict::Allow
    }
    pub fn deny(reason: impl Into<String>) -> Self {
        Verdict::Deny {
            reason: reason.into(),
        }
    }
}

// ── Filter trait ──────────────────────────────────────────────────────────────

pub trait Filter: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, content: &ClipboardContent) -> Verdict;
}

// ── Filter chain ──────────────────────────────────────────────────────────────

pub struct FilterChain {
    filters: Vec<Box<dyn Filter>>,
}

impl FilterChain {
    pub fn new() -> Self {
        Self { filters: vec![] }
    }

    /// Build a default chain from the current settings.
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        let mut chain = Self::new();

        chain.push(SizeFilter {
            max_bytes: settings.max_payload_bytes as usize,
        });

        chain.push(TypeFilter {
            allow_text: settings.sync_text,
            allow_images: settings.sync_images,
            allow_files: settings.sync_files,
        });

        chain.push(IgnorePatternFilter::from_settings(settings));
        chain.push(ExtensionFilter::default());
        chain.push(SensitiveTextFilter::from_settings(settings));
        chain
    }

    pub fn push(&mut self, f: impl Filter + 'static) {
        self.filters.push(Box::new(f));
    }

    /// Run the chain. Returns the first Deny, or Allow if all pass.
    pub fn run(&self, content: &ClipboardContent) -> Verdict {
        for f in &self.filters {
            let v = f.check(content);
            if !v.is_allow() {
                tracing::debug!("[filter:{}] {:?}", f.name(), v);
                return v;
            }
        }
        Verdict::Allow
    }
}

impl Default for FilterChain {
    fn default() -> Self {
        Self::new()
    }
}

pub struct IgnorePatternFilter {
    patterns: Vec<String>,
}

impl IgnorePatternFilter {
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        Self {
            patterns: settings
                .ignore_patterns
                .iter()
                .map(|pattern| pattern.to_lowercase())
                .filter(|pattern| !pattern.is_empty())
                .collect(),
        }
    }
}

impl Filter for IgnorePatternFilter {
    fn name(&self) -> &'static str {
        "ignore_pattern"
    }

    fn check(&self, content: &ClipboardContent) -> Verdict {
        if self.patterns.is_empty() {
            return Verdict::Allow;
        }

        let haystack = match content {
            ClipboardContent::Text(text) => text.to_lowercase(),
            ClipboardContent::File { name, .. } => name.to_lowercase(),
            ClipboardContent::Image { .. } => return Verdict::Allow,
        };

        for pattern in &self.patterns {
            if haystack.contains(pattern) {
                return Verdict::deny(format!("content matches ignored pattern '{}'", pattern));
            }
        }

        Verdict::Allow
    }
}

// ── SizeFilter ────────────────────────────────────────────────────────────────

pub struct SizeFilter {
    pub max_bytes: usize,
}

impl Filter for SizeFilter {
    fn name(&self) -> &'static str {
        "size"
    }

    fn check(&self, content: &ClipboardContent) -> Verdict {
        let len = content.byte_len();
        if len > self.max_bytes {
            Verdict::deny(format!(
                "payload {} bytes exceeds limit {} bytes",
                len, self.max_bytes
            ))
        } else {
            Verdict::Allow
        }
    }
}

// ── TypeFilter ────────────────────────────────────────────────────────────────

pub struct TypeFilter {
    pub allow_text: bool,
    pub allow_images: bool,
    pub allow_files: bool,
}

impl Filter for TypeFilter {
    fn name(&self) -> &'static str {
        "type"
    }

    fn check(&self, content: &ClipboardContent) -> Verdict {
        match content {
            ClipboardContent::Text(_) if !self.allow_text => {
                Verdict::deny("text sync disabled in settings")
            }
            ClipboardContent::Image { .. } if !self.allow_images => {
                Verdict::deny("image sync disabled in settings")
            }
            ClipboardContent::File { .. } if !self.allow_files => {
                Verdict::deny("file sync disabled in settings")
            }
            _ => Verdict::Allow,
        }
    }
}

// ── ExtensionFilter ───────────────────────────────────────────────────────────

/// Block file types that could be dangerous to auto-receive.
pub struct ExtensionFilter {
    blocked: Vec<&'static str>,
}

impl Default for ExtensionFilter {
    fn default() -> Self {
        Self {
            blocked: vec![
                // Executables
                "exe", "com", "bat", "cmd", "msi", "dll", "so", "dylib", "app", "deb", "rpm", "apk",
                "ipa", // Scripts
                "sh", "bash", "zsh", "fish", "ps1", "psm1", "vbs", "js", "ts", "py", "rb", "pl",
                "php", // Office macros
                "xlsm", "xlsb", "docm", "pptm",
            ],
        }
    }
}

impl Filter for ExtensionFilter {
    fn name(&self) -> &'static str {
        "extension"
    }

    fn check(&self, content: &ClipboardContent) -> Verdict {
        if let ClipboardContent::File { name, .. } = content {
            let ext = std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if self.blocked.contains(&ext.as_str()) {
                return Verdict::deny(format!("file extension '.{}' is blocked", ext));
            }
        }
        Verdict::Allow
    }
}

// ── SensitiveTextFilter ───────────────────────────────────────────────────────

/// Heuristic filter for common sensitive text patterns.
/// Disabled by default — must be explicitly opted into via settings.
///
/// Note: This is best-effort and will have false positives/negatives.
/// Users who need to copy passwords across devices should disable it.
pub struct SensitiveTextFilter {
    pub enabled: bool,
    patterns: Vec<&'static str>,
}

impl Default for SensitiveTextFilter {
    fn default() -> Self {
        Self {
            enabled: false,
            patterns: vec![
                "BEGIN RSA PRIVATE KEY",
                "BEGIN OPENSSH PRIVATE KEY",
                "BEGIN EC PRIVATE KEY",
                "BEGIN PGP PRIVATE KEY BLOCK",
                "-----BEGIN CERTIFICATE-----",
                // Common password manager patterns
                "password:",
                "passwd:",
                "secret:",
                "api_key:",
                "api-key:",
                "apikey:",
                "access_token:",
                "private_key:",
                "bearer ",
                "aws_secret_access_key",
                "github_pat_",
                "xoxb-",
                "xoxp-",
            ],
        }
    }
}

impl SensitiveTextFilter {
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        Self {
            enabled: settings.block_sensitive_text,
            ..Self::default()
        }
    }

    fn looks_like_secret(text: &str) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.len() < 12 {
            return false;
        }

        let single_line = !trimmed.contains('\n');
        let has_space = trimmed.contains(char::is_whitespace);
        let alpha = trimmed.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let digit = trimmed.chars().filter(|c| c.is_ascii_digit()).count();
        let punct = trimmed
            .chars()
            .filter(|c| c.is_ascii_punctuation() && *c != '_' && *c != '-')
            .count();

        single_line && !has_space && trimmed.len() >= 20 && alpha >= 4 && digit >= 2 && punct >= 1
    }
}

impl Filter for SensitiveTextFilter {
    fn name(&self) -> &'static str {
        "sensitive_text"
    }

    fn check(&self, content: &ClipboardContent) -> Verdict {
        if !self.enabled {
            return Verdict::Allow;
        }

        if let ClipboardContent::Text(text) = content {
            let lower = text.to_lowercase();
            for pat in &self.patterns {
                if lower.contains(&pat.to_lowercase()) {
                    return Verdict::deny(format!(
                        "text matches sensitive pattern '{}' (disable block_sensitive_text to override)",
                        pat
                    ));
                }
            }

            if Self::looks_like_secret(text) {
                return Verdict::deny("text looks like a secret/token and smart sync blocked it");
            }
        }
        Verdict::Allow
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ClipboardContent;

    fn text(s: &str) -> ClipboardContent {
        ClipboardContent::Text(s.into())
    }
    fn image() -> ClipboardContent {
        ClipboardContent::Image {
            mime: "image/png".into(),
            data: vec![0; 100],
        }
    }
    fn file(name: &str) -> ClipboardContent {
        ClipboardContent::File {
            name: name.into(),
            data: vec![],
        }
    }

    #[test]
    fn size_filter_allows_small() {
        let f = SizeFilter { max_bytes: 1024 };
        assert_eq!(f.check(&text("hello")), Verdict::Allow);
    }

    #[test]
    fn size_filter_denies_large() {
        let f = SizeFilter { max_bytes: 4 };
        let v = f.check(&text("hello world"));
        assert!(matches!(v, Verdict::Deny { .. }));
    }

    #[test]
    fn type_filter_respects_flags() {
        let f = TypeFilter {
            allow_text: false,
            allow_images: true,
            allow_files: true,
        };
        assert!(matches!(f.check(&text("hi")), Verdict::Deny { .. }));
        assert_eq!(f.check(&image()), Verdict::Allow);
    }

    #[test]
    fn extension_filter_blocks_exe() {
        let f = ExtensionFilter::default();
        assert!(matches!(
            f.check(&file("malware.exe")),
            Verdict::Deny { .. }
        ));
        assert_eq!(f.check(&file("report.pdf")), Verdict::Allow);
    }

    #[test]
    fn extension_filter_case_insensitive() {
        let f = ExtensionFilter::default();
        assert!(matches!(f.check(&file("VIRUS.EXE")), Verdict::Deny { .. }));
    }

    #[test]
    fn sensitive_text_filter_disabled_by_default() {
        let f = SensitiveTextFilter::default();
        // Even with a matching pattern, filter is off by default.
        assert_eq!(f.check(&text("password: hunter2")), Verdict::Allow);
    }

    #[test]
    fn sensitive_text_filter_enabled() {
        let f = SensitiveTextFilter {
            enabled: true,
            ..Default::default()
        };
        assert!(matches!(
            f.check(&text("password: hunter2")),
            Verdict::Deny { .. }
        ));
        assert_eq!(f.check(&text("hello world")), Verdict::Allow);
    }

    #[test]
    fn sensitive_text_blocks_private_key() {
        let f = SensitiveTextFilter {
            enabled: true,
            ..Default::default()
        };
        let key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAK...";
        assert!(matches!(f.check(&text(key)), Verdict::Deny { .. }));
    }

    #[test]
    fn chain_short_circuits_on_deny() {
        let mut chain = FilterChain::default();
        chain.push(SizeFilter { max_bytes: 3 });
        chain.push(SensitiveTextFilter {
            enabled: true,
            ..Default::default()
        });

        // Size violation should fire before sensitive-text check.
        let v = chain.run(&text("longer than 3 bytes"));
        assert!(matches!(v, Verdict::Deny { reason } if reason.contains("exceeds limit")));
    }

    #[test]
    fn chain_all_pass() {
        let mut chain = FilterChain::default();
        chain.push(SizeFilter { max_bytes: 1024 });
        chain.push(TypeFilter {
            allow_text: true,
            allow_images: true,
            allow_files: true,
        });
        assert_eq!(chain.run(&text("safe text")), Verdict::Allow);
    }
}
