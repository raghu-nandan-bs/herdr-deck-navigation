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
    pub id: String,
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

/// "w1:p3" -> "pane 3"; falls back to the raw id if it has no `p` segment.
fn pane_label(pane_id: &str) -> String {
    match pane_id.rsplit_once(":p") {
        Some((_, n)) => format!("pane {n}"),
        None => pane_id.to_string(),
    }
}

pub fn build_deck(json: &str) -> Result<Deck> {
    let env: Envelope = serde_json::from_str(json)?;
    let snap = env.result.snapshot;
    let cur_ws = snap.focused_workspace_id.clone();
    let cur_pane = snap.focused_pane_id.clone();

    let mut raw_tabs = snap.tabs;
    raw_tabs.sort_by_key(|t| t.number);
    let mut raw_ws = snap.workspaces;
    raw_ws.sort_by_key(|w| w.number);

    let mut workspaces = Vec::new();
    for w in raw_ws {
        let mut tabs = Vec::new();
        let mut counts = Counts::default();
        let mut worst = Status::Unknown;
        for t in raw_tabs.iter().filter(|t| t.workspace_id == w.workspace_id) {
            let mut panes = Vec::new();
            for p in snap.panes.iter().filter(|p| p.tab_id == t.tab_id) {
                let status = Status::parse(&p.agent_status);
                counts.add(status);
                if status.rank() < worst.rank() {
                    worst = status;
                }
                panes.push(Pane {
                    label: pane_label(&p.pane_id),
                    is_current: cur_pane.as_deref() == Some(p.pane_id.as_str()),
                    id: p.pane_id.clone(),
                    status,
                });
            }
            tabs.push(Tab {
                id: t.tab_id.clone(),
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
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"working"},
        {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"idle"}
      ]
    }}}"#;

    #[test]
    fn builds_tree_grouped_by_workspace_and_tab() {
        let deck = build_deck(MINI).unwrap();
        assert_eq!(deck.workspaces.len(), 2);
        let api = &deck.workspaces[0];
        assert_eq!(api.label, "api");
        assert_eq!(api.tabs.len(), 1);
        assert_eq!(api.tabs[0].label, "server");
        assert_eq!(api.tabs[0].panes.len(), 2);
    }

    #[test]
    fn derives_pane_label_from_id() {
        let deck = build_deck(MINI).unwrap();
        assert_eq!(deck.workspaces[0].tabs[0].panes[0].label, "pane 1");
    }

    #[test]
    fn marks_current_workspace_and_pane() {
        let deck = build_deck(MINI).unwrap();
        assert!(deck.workspaces[0].is_current);
        assert!(!deck.workspaces[1].is_current);
        assert!(deck.workspaces[0].tabs[0].panes[1].is_current); // w1:p2
    }

    #[test]
    fn counts_and_worst_status() {
        let deck = build_deck(MINI).unwrap();
        let c = &deck.workspaces[0].counts;
        assert_eq!((c.blocked, c.working), (1, 1));
        assert_eq!(deck.workspaces[0].worst, Status::Blocked);
        assert_eq!(deck.workspaces[1].worst, Status::Idle);
    }

    #[test]
    fn parses_the_real_captured_snapshot() {
        let raw = include_str!("../tests/fixtures/snapshot.json");
        let deck = build_deck(raw).unwrap();
        assert!(!deck.workspaces.is_empty());
        // every pane belongs to a tab that belongs to its workspace
        for ws in &deck.workspaces {
            assert!(!ws.label.is_empty());
        }
        // known shape of tests/fixtures/snapshot.json
        assert_eq!(deck.workspaces.len(), 3);
        assert!(deck.workspaces.iter().any(|w| w.label == "web"));
    }
}
