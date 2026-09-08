//! Evidence recording: every session's actions land in an append-only
//! `events.jsonl` plus snapshot files under a per-session directory, so a
//! run can be audited and replayed after the fact (the "evidence primitive"
//! hosted agent platforms ship — self-hosted here).
//!
//! Layout:
//! ```text
//! ~/.ghostcloak/recordings/<session_id>/
//! ├── identity.toml      # the persona the run used
//! ├── events.jsonl       # one JSON object per action
//! └── snapshots/         # full page snapshots (url, title, content)
//!     └── 0001-<page_id>.md
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Serialize)]
struct Event {
    ts: String,
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_id: Option<String>,
    detail: serde_json::Value,
}

#[derive(Clone)]
pub struct Recorder {
    root: PathBuf,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new(Self::default_root())
    }
}

impl Recorder {
    fn default_root() -> PathBuf {
        std::env::var_os("GHOSTFOX_RECORDINGS")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                Path::new(&home).join(".ghostfox").join("recordings")
            })
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        // Session ids are internally generated short ids; still, don't let
        // them escape the recordings root.
        let safe: String = session_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(64)
            .collect();
        self.root.join(safe)
    }

    /// Append one action to the session's event log.
    pub fn record(
        &self,
        session_id: &str,
        tool: &str,
        page_id: Option<&str>,
        detail: serde_json::Value,
    ) -> std::io::Result<()> {
        let dir = self.session_dir(session_id);
        std::fs::create_dir_all(&dir)?;
        let event = Event {
            ts: chrono::Utc::now().to_rfc3339(),
            tool: tool.to_string(),
            page_id: page_id.map(str::to_string),
            detail,
        };
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("events.jsonl"))?;
        writeln!(f, "{}", serde_json::to_string(&event).unwrap_or_default())
    }

    /// Persist the identity behind a session (provenance of the persona).
    pub fn record_identity(&self, session_id: &str, toml: &str) -> std::io::Result<()> {
        let dir = self.session_dir(session_id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("identity.toml"), toml)
    }

    /// Save a full page snapshot and log its file path.
    pub fn record_snapshot(
        &self,
        session_id: &str,
        page_id: &str,
        snapshot: &ghostcloak_core::engine::PageSnapshot,
    ) -> std::io::Result<PathBuf> {
        let dir = self.session_dir(session_id).join("snapshots");
        std::fs::create_dir_all(&dir)?;
        let n = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) + 1;
        let name = format!("{:04}-{}.md", n, sanitize(page_id));
        let body = format!(
            "# {}\n\nURL: {}\n\nCaptured: {}\n\n---\n\n{}",
            snapshot.title.as_deref().unwrap_or("(untitled)"),
            snapshot.url,
            snapshot.captured_at,
            snapshot.content
        );
        let path = dir.join(name);
        std::fs::write(&path, body)?;
        self.record(
            session_id,
            "page_snapshot",
            Some(page_id),
            serde_json::json!({
                "url": snapshot.url,
                "file": path.to_string_lossy(),
            }),
        )?;
        Ok(path)
    }

    /// Save a screenshot PNG and log its file path.
    pub fn record_screenshot(
        &self,
        session_id: &str,
        page_id: &str,
        png: &[u8],
    ) -> std::io::Result<PathBuf> {
        let dir = self.session_dir(session_id).join("screenshots");
        std::fs::create_dir_all(&dir)?;
        let n = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) + 1;
        let path = dir.join(format!("{:04}-{}.png", n, sanitize(page_id)));
        std::fs::write(&path, png)?;
        self.record(
            session_id,
            "page_screenshot",
            Some(page_id),
            serde_json::json!({ "file": path.to_string_lossy(), "bytes": png.len() }),
        )?;
        Ok(path)
    }

    /// Summary of everything recorded for a session (what the
    /// `session_evidence` tool returns).
    pub fn evidence(&self, session_id: &str) -> std::io::Result<serde_json::Value> {
        let dir = self.session_dir(session_id);
        let events: Vec<serde_json::Value> = std::fs::read_to_string(dir.join("events.jsonl"))?
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let mut snapshots = Vec::new();
        let snap_dir = dir.join("snapshots");
        if snap_dir.is_dir() {
            for entry in std::fs::read_dir(&snap_dir)? {
                let p = entry?.path();
                snapshots.push(p.to_string_lossy().to_string());
            }
            snapshots.sort();
        }
        let identity = dir
            .join("identity.toml")
            .is_file()
            .then(|| dir.join("identity.toml").to_string_lossy().to_string());
        Ok(serde_json::json!({
            "session_id": session_id,
            "dir": dir.to_string_lossy(),
            "identity_toml": identity,
            "event_count": events.len(),
            "events": events,
            "snapshots": snapshots,
        }))
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect()
}
