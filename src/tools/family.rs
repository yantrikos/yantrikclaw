//! Tool family routing — semantic grouping for tier-aware tool selection.
//!
//! Instead of exposing 30+ tools to a small model, route the user's query
//! to the best-matching family first, then expose only that family's tools.
//! Ported from yantrik-ml/src/capability.rs.

use serde::{Deserialize, Serialize};

/// Semantic tool families for capability-family routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolFamily {
    /// Email, messaging, notifications.
    Communicate,
    /// Calendar events, scheduling, cron jobs.
    Schedule,
    /// Memory recall, store, search, notes.
    Remember,
    /// Web search, fetch, browse, extract.
    Browse,
    /// Read, write, edit, search files and code.
    Files,
    /// System commands, processes, utilities, security.
    System,
    /// Sub-agents, delegation, complex multi-step tasks.
    Delegate,
    /// Weather, integrations, external services.
    World,
}

impl ToolFamily {
    /// All families as a slice.
    pub const ALL: &[ToolFamily] = &[
        ToolFamily::Communicate,
        ToolFamily::Schedule,
        ToolFamily::Remember,
        ToolFamily::Browse,
        ToolFamily::Files,
        ToolFamily::System,
        ToolFamily::Delegate,
        ToolFamily::World,
    ];

    /// Keywords that map to this family (for lightweight routing).
    pub fn keywords(&self) -> &'static [&'static str] {
        match self {
            ToolFamily::Communicate => &[
                "email",
                "mail",
                "inbox",
                "send",
                "reply",
                "message",
                "whatsapp",
                "telegram",
                "notify",
                "notification",
                "draft",
            ],
            ToolFamily::Schedule => &[
                "calendar",
                "event",
                "meeting",
                "schedule",
                "appointment",
                "today",
                "tomorrow",
                "free time",
                "busy",
                "agenda",
                "cron",
                "recurring",
            ],
            ToolFamily::Remember => &[
                "remember",
                "recall",
                "memory",
                "memories",
                "forget",
                "note",
                "notes",
                "what did",
                "preference",
            ],
            ToolFamily::Browse => &[
                "search",
                "browse",
                "website",
                "web",
                "url",
                "http",
                "fetch",
                "download",
                "look up",
                "find online",
                "google",
            ],
            ToolFamily::Files => &[
                "file",
                "read",
                "write",
                "directory",
                "folder",
                "edit",
                "grep",
                "glob",
                "code",
                "script",
                "save file",
                "git",
            ],
            ToolFamily::System => &[
                "system",
                "process",
                "disk",
                "cpu",
                "reminder",
                "timer",
                "alarm",
                "uptime",
                "run command",
                "execute",
                "screenshot",
                "vault",
                "password",
                "credential",
                "secret",
                "calculate",
            ],
            ToolFamily::Delegate => &[
                "parallel",
                "simultaneously",
                "multiple tasks",
                "spawn",
                "complex",
                "analyze deeply",
                "think hard",
                "delegate",
                "swarm",
            ],
            ToolFamily::World => &[
                "weather",
                "temperature",
                "forecast",
                "rain",
                "news",
                "jira",
                "notion",
                "linkedin",
                "cloud",
                "backup",
                "composio",
                "integration",
            ],
        }
    }

    /// YantrikClaw tool categories that belong to this family.
    pub fn categories(&self) -> &'static [&'static str] {
        match self {
            ToolFamily::Communicate => &["integrate"],
            ToolFamily::Schedule => &["schedule"],
            ToolFamily::Remember => &["memory"],
            ToolFamily::Browse => &["browse"],
            ToolFamily::Files => &["files", "git"],
            ToolFamily::System => &[
                "system", "utility", "media", "security", "hardware", "config", "sop",
            ],
            ToolFamily::Delegate => &["agent"],
            ToolFamily::World => &["integrate", "cloud", "notify"],
        }
    }

    /// Route a query to the best-matching families using keyword matching.
    /// Returns families sorted by match score (best first), only those with score > 0.
    pub fn route_query(query: &str) -> Vec<(ToolFamily, f64)> {
        let query_lower = query.to_lowercase();
        let mut scores: Vec<(ToolFamily, f64)> = ToolFamily::ALL
            .iter()
            .map(|&family| {
                let keywords = family.keywords();
                let matches = keywords
                    .iter()
                    .filter(|kw| query_lower.contains(**kw))
                    .count();
                let score = matches as f64 / keywords.len() as f64;
                (family, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    /// Get the single best family for a query, or None if no keywords match.
    pub fn best_for_query(query: &str) -> Option<ToolFamily> {
        Self::route_query(query).first().map(|(f, _)| *f)
    }

    /// Check if a tool category belongs to this family.
    pub fn matches_category(&self, category: &str) -> bool {
        self.categories().contains(&category)
    }
}

impl std::fmt::Display for ToolFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolFamily::Communicate => write!(f, "COMMUNICATE"),
            ToolFamily::Schedule => write!(f, "SCHEDULE"),
            ToolFamily::Remember => write!(f, "REMEMBER"),
            ToolFamily::Browse => write!(f, "BROWSE"),
            ToolFamily::Files => write!(f, "FILES"),
            ToolFamily::System => write!(f, "SYSTEM"),
            ToolFamily::Delegate => write!(f, "DELEGATE"),
            ToolFamily::World => write!(f, "WORLD"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_email_query_to_communicate() {
        let families = ToolFamily::route_query("send an email to alice");
        assert!(!families.is_empty());
        assert_eq!(families[0].0, ToolFamily::Communicate);
    }

    #[test]
    fn route_file_query_to_files() {
        let families = ToolFamily::route_query("read the config file");
        assert!(!families.is_empty());
        assert_eq!(families[0].0, ToolFamily::Files);
    }

    #[test]
    fn route_memory_query_to_remember() {
        let families = ToolFamily::route_query("what did I say about my preference");
        assert!(!families.is_empty());
        assert_eq!(families[0].0, ToolFamily::Remember);
    }

    #[test]
    fn route_web_query_to_browse() {
        let families = ToolFamily::route_query("search the web for rust tutorials");
        assert!(!families.is_empty());
        assert_eq!(families[0].0, ToolFamily::Browse);
    }

    #[test]
    fn route_git_query_to_files() {
        let families = ToolFamily::route_query("git status of the repo");
        assert!(!families.is_empty());
        assert_eq!(families[0].0, ToolFamily::Files);
    }

    #[test]
    fn no_match_returns_empty() {
        let families = ToolFamily::route_query("hello how are you");
        assert!(families.is_empty());
    }

    #[test]
    fn best_for_query_returns_top() {
        assert_eq!(
            ToolFamily::best_for_query("schedule a meeting"),
            Some(ToolFamily::Schedule)
        );
    }

    #[test]
    fn category_matching() {
        assert!(ToolFamily::Files.matches_category("files"));
        assert!(ToolFamily::Files.matches_category("git"));
        assert!(!ToolFamily::Files.matches_category("browse"));
    }
}
