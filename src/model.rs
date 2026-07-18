use crate::theme::Status;
use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct Envelope {
    result: ResultBody,
}
#[derive(Deserialize)]
struct ResultBody {
    snapshot: RawSnapshot,
}
#[derive(Deserialize)]
struct RawSnapshot {
    #[serde(default)]
    focused_workspace_id: Option<String>,
    #[serde(default)]
    focused_pane_id: Option<String>,
    #[serde(default)]
    workspaces: Vec<RawWorkspace>,
    #[serde(default)]
    tabs: Vec<RawTab>,
    #[serde(default)]
    panes: Vec<RawPane>,
}
#[derive(Deserialize)]
struct RawWorkspace {
    workspace_id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    number: u32,
}
#[derive(Deserialize)]
struct RawTab {
    tab_id: String,
    workspace_id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    number: u32,
}
#[derive(Deserialize)]
struct RawPane {
    pane_id: String,
    tab_id: String,
    #[serde(default)]
    agent_status: String,
    /// Human label herdr assigns (agent name, terminal title, …). Absent for plain shells.
    #[serde(default)]
    label: Option<String>,
}

/// Invocation context from herdr's env, used to (a) drop Deck's own overlay pane
/// and (b) mark the workspace/pane the user was actually on — the live snapshot's
/// `focused_*` point at Deck itself, since the overlay steals focus.
#[derive(Default, Clone)]
pub struct Context {
    pub self_pane_id: Option<String>,
    pub current_workspace_id: Option<String>,
    pub current_pane_id: Option<String>,
}

impl Context {
    pub fn from_env() -> Context {
        Context {
            self_pane_id: env_nonempty("HERDR_PANE_ID"),
            current_workspace_id: env_nonempty("HERDR_WORKSPACE_ID"),
            current_pane_id: std::env::var("HERDR_PLUGIN_CONTEXT_JSON")
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    v.get("focused_pane_id")
                        .and_then(|x| x.as_str().map(str::to_string))
                }),
        }
    }
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

#[derive(Clone, Default)]
pub struct Counts {
    pub blocked: usize,
    pub working: usize,
    pub done: usize,
    pub idle: usize,
    pub unknown: usize,
}
impl Counts {
    fn add(&mut self, s: Status) {
        match s {
            Status::Blocked => self.blocked += 1,
            Status::Working => self.working += 1,
            Status::Done => self.done += 1,
            Status::Idle => self.idle += 1,
            Status::Unknown => self.unknown += 1,
        }
    }
}

pub struct Pane {
    pub id: String,
    pub label: String,
    pub status: Status,
    pub is_current: bool,
}
pub struct Tab {
    pub label: String,
    pub panes: Vec<Pane>,
}
pub struct Workspace {
    pub id: String,
    pub label: String,
    pub is_current: bool,
    pub tabs: Vec<Tab>,
    pub counts: Counts,
    pub worst: Status,
}
pub struct Deck {
    pub workspaces: Vec<Workspace>,
}

pub fn build_deck(json: &str, ctx: &Context) -> Result<Deck> {
    let env: Envelope = serde_json::from_str(json)?;
    let snap = env.result.snapshot;
    // Prefer the env-provided current ids; fall back to the snapshot's focused ids.
    let cur_ws = ctx
        .current_workspace_id
        .clone()
        .or(snap.focused_workspace_id);
    let cur_pane = ctx.current_pane_id.clone().or(snap.focused_pane_id);
    let self_pane = ctx.self_pane_id.as_deref();

    let mut raw_tabs = snap.tabs;
    raw_tabs.sort_by_key(|t| t.number);
    let mut raw_ws = snap.workspaces;
    raw_ws.sort_by_key(|w| w.number);

    let mut workspaces = Vec::new();
    for w in raw_ws {
        let mut tabs = Vec::new();
        let mut counts = Counts::default();
        let mut worst = Status::Unknown;
        let mut pane_no = 0u32; // per-workspace counter for unlabeled panes
        for t in raw_tabs.iter().filter(|t| t.workspace_id == w.workspace_id) {
            let mut panes = Vec::new();
            for p in snap.panes.iter().filter(|p| p.tab_id == t.tab_id) {
                if self_pane == Some(p.pane_id.as_str()) {
                    continue; // drop Deck's own overlay pane
                }
                pane_no += 1;
                let status = Status::parse(&p.agent_status);
                counts.add(status);
                if status.rank() < worst.rank() {
                    worst = status;
                }
                let label = p
                    .label
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("pane {pane_no}"));
                panes.push(Pane {
                    label,
                    is_current: cur_pane.as_deref() == Some(p.pane_id.as_str()),
                    id: p.pane_id.clone(),
                    status,
                });
            }
            tabs.push(Tab {
                label: t.label.clone(),
                panes,
            });
        }
        workspaces.push(Workspace {
            is_current: cur_ws.as_deref() == Some(w.workspace_id.as_str()),
            id: w.workspace_id,
            label: w.label,
            tabs,
            counts,
            worst,
        });
    }
    Ok(Deck { workspaces })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Status;

    const MINI: &str = r#"
    {"id":"x","result":{"type":"session_snapshot","snapshot":{
      "focused_workspace_id":"w1","focused_tab_id":"w1:t1","focused_pane_id":"w1:p2",
      "workspaces":[
        {"workspace_id":"w1","label":"api","number":1,"active_tab_id":"w1:t1"},
        {"workspace_id":"w2","label":"web","number":2,"active_tab_id":"w2:t1"}
      ],
      "tabs":[
        {"tab_id":"w1:t1","workspace_id":"w1","label":"server","number":1},
        {"tab_id":"w2:t1","workspace_id":"w2","label":"ui","number":1}
      ],
      "panes":[
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"working","label":"loadtest agent"},
        {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"idle"}
      ]
    }}}"#;

    fn deck() -> Deck {
        build_deck(MINI, &Context::default()).unwrap()
    }

    #[test]
    fn builds_tree_grouped_by_workspace_and_tab() {
        let d = deck();
        assert_eq!(d.workspaces.len(), 2);
        let api = &d.workspaces[0];
        assert_eq!(api.label, "api");
        assert_eq!(api.tabs.len(), 1);
        assert_eq!(api.tabs[0].label, "server");
        assert_eq!(api.tabs[0].panes.len(), 2);
    }

    #[test]
    fn uses_pane_label_when_present_else_numbers() {
        let d = deck();
        // p1 carries a label; p2 does not, so it falls back to a positional number.
        assert_eq!(d.workspaces[0].tabs[0].panes[0].label, "loadtest agent");
        assert_eq!(d.workspaces[0].tabs[0].panes[1].label, "pane 2");
    }

    #[test]
    fn marks_current_workspace_and_pane_from_snapshot_by_default() {
        let d = deck();
        assert!(d.workspaces[0].is_current);
        assert!(!d.workspaces[1].is_current);
        assert!(d.workspaces[0].tabs[0].panes[1].is_current); // w1:p2
    }

    #[test]
    fn context_overrides_current_markers() {
        let ctx = Context {
            self_pane_id: None,
            current_workspace_id: Some("w2".into()),
            current_pane_id: Some("w2:p1".into()),
        };
        let d = build_deck(MINI, &ctx).unwrap();
        assert!(!d.workspaces[0].is_current);
        assert!(d.workspaces[1].is_current);
        assert!(d.workspaces[1].tabs[0].panes[0].is_current);
    }

    #[test]
    fn excludes_self_overlay_pane() {
        let ctx = Context {
            self_pane_id: Some("w1:p2".into()),
            ..Default::default()
        };
        let d = build_deck(MINI, &ctx).unwrap();
        // w1:p2 is gone; only the labeled pane remains in that tab
        assert_eq!(d.workspaces[0].tabs[0].panes.len(), 1);
        assert_eq!(d.workspaces[0].tabs[0].panes[0].label, "loadtest agent");
        assert_eq!(d.workspaces[0].counts.blocked, 0);
    }

    #[test]
    fn counts_and_worst_status() {
        let d = deck();
        let c = &d.workspaces[0].counts;
        assert_eq!((c.blocked, c.working), (1, 1));
        assert_eq!(d.workspaces[0].worst, Status::Blocked);
        assert_eq!(d.workspaces[1].worst, Status::Idle);
    }

    #[test]
    fn parses_the_real_captured_snapshot() {
        let raw = include_str!("../tests/fixtures/snapshot.json");
        let d = build_deck(raw, &Context::default()).unwrap();
        assert_eq!(d.workspaces.len(), 3);
        for ws in &d.workspaces {
            assert!(!ws.label.is_empty());
        }
        assert!(d.workspaces.iter().any(|w| w.label == "web"));
    }
}
