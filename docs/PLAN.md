# Deck — Card-Style Workspace Navigator · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a standalone herdr plugin — a Rust TUI (`herdr-deck`) launched in a modal popup — that replaces the flat navigator with a horizontal deck of workspace cards, each a status dashboard, navigated with `←/→` between workspaces and `↑/↓` within one.

**Architecture:** herdr binds `prefix+g` → a plugin action that opens a popup pane running `herdr-deck`. The binary reads `session.snapshot` over the newline-delimited-JSON Unix socket (`HERDR_SOCKET_PATH`), builds an in-memory deck model, renders it with ratatui, and on `Enter` issues `workspace.focus` / `tab.focus` / `pane.focus` before exiting.

**Tech Stack:** Rust 2021, `ratatui` 0.29, `crossterm` 0.29, `serde` / `serde_json`, `anyhow`. Unix domain socket via std `UnixStream`.

## Global Constraints

- `min_herdr_version = "0.7.0"`; developed against `0.7.3` (socket protocol 16).
- Plugin `id = "deck"`, binary `herdr-deck`, popup entrypoint `picker`, action `open`, keybind `prefix+g`.
- Socket wire format: one JSON object per line, `\n`-terminated. Request `{"id":<str>,"method":<str>,"params":<obj>}`; `params` is REQUIRED (send `{}` when empty). Response `{"id":..,"result":..}` or `{"id":..,"error":{"code","message"}}`.
- `agent_status` enum (verbatim): `blocked`, `working`, `done`, `idle`, `unknown`.
- Status precedence for the card "worst" stripe (0 = worst): `blocked` < `working` < `done` < `idle` < `unknown`.
- Glyphs / colors (Catppuccin Mocha): blocked `◉` red `#f38ba8`; working `◍` yellow `#f9e2af`; done `●` teal `#94e2d5`; idle `✓` green `#a6e3a1`; unknown `○` overlay0 `#6c7086`. Accent/selection `peach #fab387`; base `#1e1e2e`; text `#cdd6f4`.
- Platforms v1: `["linux","macos"]`. Windows named-pipe transport is a documented follow-up (§ Task 5 note).
- TDD, DRY, YAGNI, frequent commits.

---

### Task 1: Project scaffold + compiling binary

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore` (already present — verify `/target` ignored)

**Interfaces:**
- Produces: a buildable crate named `herdr-deck` with a `main()` that exits 0.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "herdr-deck"
version = "0.1.0"
edition = "2021"
description = "Card-style workspace navigator plugin for herdr"
license = "MIT"

[[bin]]
name = "herdr-deck"
path = "src/main.rs"

[dependencies]
ratatui = "0.29"
crossterm = "0.29"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

- [ ] **Step 2: Write a placeholder `src/main.rs`**

```rust
fn main() -> anyhow::Result<()> {
    Ok(())
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles cleanly, produces `target/debug/herdr-deck`.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/main.rs .gitignore
git commit -m "chore: scaffold herdr-deck crate"
```

---

### Task 2: Status model (theme.rs)

**Files:**
- Create: `src/theme.rs`
- Modify: `src/main.rs` (add `mod theme;`)

**Interfaces:**
- Produces:
  - `enum Status { Blocked, Working, Done, Idle, Unknown }`
  - `Status::parse(&str) -> Status` (unknown for unrecognized)
  - `Status::rank(self) -> u8` (blocked=0 … unknown=4)
  - `Status::glyph(self) -> &'static str`
  - `Status::color(self) -> ratatui::style::Color`
  - color consts: `BASE, TEXT, OVERLAY0, ACCENT, RED, YELLOW, TEAL, GREEN` (ratatui `Color::Rgb`)

- [ ] **Step 1: Write the failing test** (append to `src/theme.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_and_unknown() {
        assert_eq!(Status::parse("blocked"), Status::Blocked);
        assert_eq!(Status::parse("working"), Status::Working);
        assert_eq!(Status::parse("done"), Status::Done);
        assert_eq!(Status::parse("idle"), Status::Idle);
        assert_eq!(Status::parse("unknown"), Status::Unknown);
        assert_eq!(Status::parse("garbage"), Status::Unknown);
    }

    #[test]
    fn worst_status_ranks_blocked_first() {
        let mut v = [Status::Idle, Status::Blocked, Status::Working];
        v.sort_by_key(|s| s.rank());
        assert_eq!(v[0], Status::Blocked);
    }

    #[test]
    fn glyphs_match_spec() {
        assert_eq!(Status::Blocked.glyph(), "◉");
        assert_eq!(Status::Working.glyph(), "◍");
        assert_eq!(Status::Done.glyph(), "●");
        assert_eq!(Status::Idle.glyph(), "✓");
        assert_eq!(Status::Unknown.glyph(), "○");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib theme`
Expected: FAIL — `Status` not defined.

- [ ] **Step 3: Write the implementation** (prepend to `src/theme.rs`)

```rust
use ratatui::style::Color;

pub const BASE: Color = Color::Rgb(0x1e, 0x1e, 0x2e);
pub const TEXT: Color = Color::Rgb(0xcd, 0xd6, 0xf4);
pub const OVERLAY0: Color = Color::Rgb(0x6c, 0x70, 0x86);
pub const SURFACE1: Color = Color::Rgb(0x45, 0x47, 0x5a);
pub const ACCENT: Color = Color::Rgb(0xfa, 0xb3, 0x87); // peach
pub const RED: Color = Color::Rgb(0xf3, 0x8b, 0xa8);
pub const YELLOW: Color = Color::Rgb(0xf9, 0xe2, 0xaf);
pub const TEAL: Color = Color::Rgb(0x94, 0xe2, 0xd5);
pub const GREEN: Color = Color::Rgb(0xa6, 0xe3, 0xa1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Blocked,
    Working,
    Done,
    Idle,
    Unknown,
}

impl Status {
    pub fn parse(s: &str) -> Status {
        match s {
            "blocked" => Status::Blocked,
            "working" => Status::Working,
            "done" => Status::Done,
            "idle" => Status::Idle,
            _ => Status::Unknown,
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Status::Blocked => 0,
            Status::Working => 1,
            Status::Done => 2,
            Status::Idle => 3,
            Status::Unknown => 4,
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Status::Blocked => "◉",
            Status::Working => "◍",
            Status::Done => "●",
            Status::Idle => "✓",
            Status::Unknown => "○",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Status::Blocked => RED,
            Status::Working => YELLOW,
            Status::Done => TEAL,
            Status::Idle => GREEN,
            Status::Unknown => OVERLAY0,
        }
    }
}
```

Add to `src/main.rs` (top): `mod theme;`

- [ ] **Step 4: Run tests**

Run: `cargo test --lib theme`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/theme.rs src/main.rs
git commit -m "feat: status enum with glyphs, colors, worst-rank"
```

---

### Task 3: Snapshot parsing + deck model (model.rs)

**Files:**
- Create: `src/model.rs`
- Modify: `src/main.rs` (add `mod model;`)
- Test fixture (already captured): `tests/fixtures/snapshot.json`

**Interfaces:**
- Consumes: `theme::Status`.
- Produces:
  - `struct Counts { pub blocked: usize, pub working: usize, pub done: usize, pub idle: usize, pub unknown: usize }`
  - `struct Pane { pub id: String, pub label: String, pub status: Status, pub is_current: bool }`
  - `struct Tab { pub id: String, pub label: String, pub panes: Vec<Pane> }`
  - `struct Workspace { pub id: String, pub label: String, pub is_current: bool, pub active_tab_id: Option<String>, pub tabs: Vec<Tab>, pub counts: Counts, pub worst: Status }`
  - `struct Deck { pub workspaces: Vec<Workspace> }`
  - `fn build_deck(snapshot_response_json: &str) -> anyhow::Result<Deck>`

- [ ] **Step 1: Write the failing tests** (append to `src/model.rs`)

```rust
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
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib model`
Expected: FAIL — `build_deck` not defined.

- [ ] **Step 3: Write the implementation** (prepend to `src/model.rs`)

```rust
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
    #[serde(default)]
    active_tab_id: Option<String>,
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
    pub active_tab_id: Option<String>,
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
            active_tab_id: w.active_tab_id,
            tabs,
            counts,
            worst,
        });
    }
    Ok(Deck { workspaces })
}
```

Add to `src/main.rs`: `mod model;`

- [ ] **Step 4: Run tests**

Run: `cargo test --lib model`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/model.rs src/main.rs
git commit -m "feat: parse session.snapshot into a deck model with rollups"
```

---

### Task 4: Navigation state machine (state.rs)

**Files:**
- Create: `src/state.rs`
- Modify: `src/main.rs` (add `mod state;`)

**Interfaces:**
- Consumes: `model::{Deck, Workspace}`, `theme::Status`.
- Produces:
  - `enum Row { Workspace, Tab(usize), Pane(usize, usize) }` (`(tab_idx)`, `(tab_idx, pane_idx)`)
  - `fn workspace_rows(ws: &Workspace) -> Vec<Row>` (row 0 = `Workspace`, then each tab + its panes)
  - `enum FocusTarget { Workspace(String), Tab(String), Pane(String) }`
  - `enum Outcome { Redraw, Focus(FocusTarget), Quit }`
  - `struct NavState { pub active: usize, pub sel: Vec<usize>, pub filter: Option<Status> }`
  - `NavState::new(deck: &Deck) -> NavState`
  - `NavState::selected_target(&self, deck: &Deck) -> Option<FocusTarget>`
  - `NavState::on_key(&mut self, deck: &Deck, code: crossterm::event::KeyCode) -> Outcome`

- [ ] **Step 1: Write the failing tests** (append to `src/state.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::build_deck;
    use crossterm::event::KeyCode;

    const MINI: &str = r#"
    {"id":"x","result":{"type":"session_snapshot","snapshot":{
      "focused_workspace_id":"w2","focused_tab_id":"w2:t1","focused_pane_id":"w2:p1",
      "workspaces":[
        {"workspace_id":"w1","label":"api","number":1,"active_tab_id":"w1:t1"},
        {"workspace_id":"w2","label":"web","number":2,"active_tab_id":"w2:t1"}
      ],
      "tabs":[
        {"tab_id":"w1:t1","workspace_id":"w1","label":"server","number":1},
        {"tab_id":"w2:t1","workspace_id":"w2","label":"ui","number":1}
      ],
      "panes":[
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"working"}
      ]
    }}}"#;

    #[test]
    fn starts_on_current_workspace() {
        let deck = build_deck(MINI).unwrap();
        let st = NavState::new(&deck);
        assert_eq!(st.active, 1); // w2 is focused
    }

    #[test]
    fn rows_start_with_workspace_then_tabs_and_panes() {
        let deck = build_deck(MINI).unwrap();
        let rows = workspace_rows(&deck.workspaces[0]);
        assert_eq!(rows[0], Row::Workspace);
        assert_eq!(rows[1], Row::Tab(0));
        assert_eq!(rows[2], Row::Pane(0, 0));
    }

    #[test]
    fn left_right_move_between_cards_clamped() {
        let deck = build_deck(MINI).unwrap();
        let mut st = NavState::new(&deck);
        st.active = 1;
        assert!(matches!(st.on_key(&deck, KeyCode::Left), Outcome::Redraw));
        assert_eq!(st.active, 0);
        st.on_key(&deck, KeyCode::Left); // clamp at 0
        assert_eq!(st.active, 0);
        st.on_key(&deck, KeyCode::Right);
        assert_eq!(st.active, 1);
    }

    #[test]
    fn up_down_move_within_active_card_clamped() {
        let deck = build_deck(MINI).unwrap();
        let mut st = NavState::new(&deck);
        st.active = 0;
        st.sel[0] = 0;
        st.on_key(&deck, KeyCode::Down);
        assert_eq!(st.sel[0], 1);
        st.on_key(&deck, KeyCode::Char('j'));
        assert_eq!(st.sel[0], 2);
        // clamp at last row (3 rows: ws, tab, pane)
        st.on_key(&deck, KeyCode::Down);
        assert_eq!(st.sel[0], 2);
        st.on_key(&deck, KeyCode::Up);
        assert_eq!(st.sel[0], 1);
    }

    #[test]
    fn enter_on_pane_focuses_that_pane() {
        let deck = build_deck(MINI).unwrap();
        let mut st = NavState::new(&deck);
        st.active = 0;
        st.sel[0] = 2; // the pane row
        match st.on_key(&deck, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Pane(id)) => assert_eq!(id, "w1:p1"),
            other => panic!("expected pane focus, got {other:?}"),
        }
    }

    #[test]
    fn enter_on_workspace_row_focuses_workspace() {
        let deck = build_deck(MINI).unwrap();
        let mut st = NavState::new(&deck);
        st.active = 0;
        st.sel[0] = 0;
        match st.on_key(&deck, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Workspace(id)) => assert_eq!(id, "w1"),
            other => panic!("expected ws focus, got {other:?}"),
        }
    }

    #[test]
    fn esc_quits() {
        let deck = build_deck(MINI).unwrap();
        let mut st = NavState::new(&deck);
        assert!(matches!(st.on_key(&deck, KeyCode::Esc), Outcome::Quit));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib state`
Expected: FAIL — `NavState` not defined.

- [ ] **Step 3: Write the implementation** (prepend to `src/state.rs`)

```rust
use crate::model::{Deck, Workspace};
use crate::theme::Status;
use crossterm::event::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Workspace,
    Tab(usize),
    Pane(usize, usize),
}

#[derive(Debug)]
pub enum FocusTarget {
    Workspace(String),
    Tab(String),
    Pane(String),
}

#[derive(Debug)]
pub enum Outcome {
    Redraw,
    Focus(FocusTarget),
    Quit,
}

pub fn workspace_rows(ws: &Workspace) -> Vec<Row> {
    let mut rows = vec![Row::Workspace];
    for (ti, tab) in ws.tabs.iter().enumerate() {
        rows.push(Row::Tab(ti));
        for pi in 0..tab.panes.len() {
            rows.push(Row::Pane(ti, pi));
        }
    }
    rows
}

pub struct NavState {
    pub active: usize,
    pub sel: Vec<usize>,
    pub filter: Option<Status>,
}

impl NavState {
    pub fn new(deck: &Deck) -> NavState {
        let active = deck
            .workspaces
            .iter()
            .position(|w| w.is_current)
            .unwrap_or(0);
        let sel = deck
            .workspaces
            .iter()
            .map(|w| {
                // start on the current pane's row if present, else row 0
                workspace_rows(w)
                    .iter()
                    .position(|r| matches!(r, Row::Pane(ti, pi) if w.tabs[*ti].panes[*pi].is_current))
                    .unwrap_or(0)
            })
            .collect();
        NavState {
            active,
            sel,
            filter: None,
        }
    }

    fn active_row_count(&self, deck: &Deck) -> usize {
        workspace_rows(&deck.workspaces[self.active]).len()
    }

    pub fn selected_target(&self, deck: &Deck) -> Option<FocusTarget> {
        let ws = deck.workspaces.get(self.active)?;
        let rows = workspace_rows(ws);
        match rows.get(self.sel[self.active])? {
            Row::Workspace => Some(FocusTarget::Workspace(ws.id.clone())),
            Row::Tab(ti) => Some(FocusTarget::Tab(ws.tabs[*ti].id.clone())),
            Row::Pane(ti, pi) => Some(FocusTarget::Pane(ws.tabs[*ti].panes[*pi].id.clone())),
        }
    }

    pub fn on_key(&mut self, deck: &Deck, code: KeyCode) -> Outcome {
        match code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.active = self.active.saturating_sub(1);
                self.clamp_sel(deck);
                Outcome::Redraw
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.active = (self.active + 1).min(deck.workspaces.len().saturating_sub(1));
                self.clamp_sel(deck);
                Outcome::Redraw
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let last = self.active_row_count(deck).saturating_sub(1);
                self.sel[self.active] = (self.sel[self.active] + 1).min(last);
                Outcome::Redraw
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel[self.active] = self.sel[self.active].saturating_sub(1);
                Outcome::Redraw
            }
            KeyCode::Enter => match self.selected_target(deck) {
                Some(t) => Outcome::Focus(t),
                None => Outcome::Redraw,
            },
            KeyCode::Char('b') => self.toggle(Status::Blocked),
            KeyCode::Char('w') => self.toggle(Status::Working),
            KeyCode::Char('d') => self.toggle(Status::Done),
            KeyCode::Char('i') => self.toggle(Status::Idle),
            KeyCode::Esc => Outcome::Quit,
            _ => Outcome::Redraw,
        }
    }

    fn toggle(&mut self, s: Status) -> Outcome {
        self.filter = if self.filter == Some(s) { None } else { Some(s) };
        Outcome::Redraw
    }

    fn clamp_sel(&mut self, deck: &Deck) {
        let last = self.active_row_count(deck).saturating_sub(1);
        self.sel[self.active] = self.sel[self.active].min(last);
    }
}
```

Add to `src/main.rs`: `mod state;`

- [ ] **Step 4: Run tests**

Run: `cargo test --lib state`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add src/state.rs src/main.rs
git commit -m "feat: navigation state machine (deck + within-card, clamped)"
```

---

### Task 5: Socket client (client.rs)

**Files:**
- Create: `src/client.rs`
- Modify: `src/main.rs` (add `mod client;`)

**Interfaces:**
- Produces:
  - `fn socket_path() -> anyhow::Result<std::path::PathBuf>` (env `HERDR_SOCKET_PATH`, else `~/.config/herdr/herdr.sock`)
  - `fn call(path: &Path, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value>` (returns the `result` object; errors on an `error` response)
  - `fn snapshot(path: &Path) -> anyhow::Result<String>` (returns the full response JSON string for `build_deck`)
  - `fn focus(path: &Path, target: &crate::state::FocusTarget) -> anyhow::Result<()>`

> **Windows follow-up:** this module uses `std::os::unix::net::UnixStream` behind `#[cfg(unix)]`. A `#[cfg(windows)]` named-pipe implementation with the same `call` signature is the v1.1 cross-platform task; the manifest lists only linux/macos until then.

- [ ] **Step 1: Write the failing test** (append to `src/client.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encodes_request_as_newline_delimited_json() {
        let line = encode_request("req1", "session.snapshot", json!({}));
        assert_eq!(line, "{\"id\":\"req1\",\"method\":\"session.snapshot\",\"params\":{}}\n");
    }

    #[test]
    fn extracts_result_or_errors() {
        let ok = decode_response("{\"id\":\"x\",\"result\":{\"type\":\"pong\"}}").unwrap();
        assert_eq!(ok["type"], "pong");
        let err = decode_response("{\"id\":\"x\",\"error\":{\"code\":\"bad\",\"message\":\"nope\"}}");
        assert!(err.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib client`
Expected: FAIL — `encode_request` / `decode_response` not defined.

- [ ] **Step 3: Write the implementation** (prepend to `src/client.rs`)

```rust
use crate::state::FocusTarget;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub fn socket_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
        if !p.is_empty() {
            return Ok(PathBuf::from(p));
        }
    }
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".config/herdr/herdr.sock"))
}

pub(crate) fn encode_request(id: &str, method: &str, params: Value) -> String {
    // serde_json preserves object key insertion order for these small literals,
    // but we build the string deterministically to keep the wire format stable.
    format!(
        "{}\n",
        serde_json::to_string(&json!({"id": id, "method": method, "params": params})).unwrap()
    )
}

pub(crate) fn decode_response(line: &str) -> Result<Value> {
    let v: Value = serde_json::from_str(line).context("invalid JSON response")?;
    if let Some(err) = v.get("error") {
        return Err(anyhow!("herdr error: {}", err));
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| anyhow!("response missing `result`"))
}

#[cfg(unix)]
pub fn call(path: &Path, method: &str, params: Value) -> Result<Value> {
    use std::os::unix::net::UnixStream;
    let stream = UnixStream::connect(path)
        .with_context(|| format!("connect {}", path.display()))?;
    let mut writer = stream.try_clone()?;
    writer.write_all(encode_request("deck", method, params).as_bytes())?;
    writer.flush()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    decode_response(line.trim_end())
}

/// Returns the full JSON response string (envelope) for `model::build_deck`.
pub fn snapshot(path: &Path) -> Result<String> {
    let result = call(path, "session.snapshot", json!({}))?;
    // build_deck expects the `{result:{snapshot:..}}` envelope shape.
    Ok(serde_json::to_string(&json!({ "result": result }))?)
}

pub fn focus(path: &Path, target: &FocusTarget) -> Result<()> {
    let (method, params) = match target {
        FocusTarget::Workspace(id) => ("workspace.focus", json!({ "workspace_id": id })),
        FocusTarget::Tab(id) => ("tab.focus", json!({ "tab_id": id })),
        FocusTarget::Pane(id) => ("pane.focus", json!({ "pane_id": id })),
    };
    call(path, method, params)?;
    Ok(())
}
```

Add to `src/main.rs`: `mod client;`

> Note: `snapshot()` wraps the raw `result` back into a `{result:{snapshot:..}}` envelope so `build_deck` (Task 3) can parse it unchanged. `call` returns the inner `result`, whose `snapshot` field is what `build_deck` reaches via `env.result.snapshot`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib client`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/client.rs src/main.rs
git commit -m "feat: newline-JSON unix socket client (snapshot + focus)"
```

---

### Task 6: Rendering (ui.rs)

**Files:**
- Create: `src/ui.rs`
- Modify: `src/main.rs` (add `mod ui;`)

**Interfaces:**
- Consumes: `model::Deck`, `state::{NavState, Row, workspace_rows}`, `theme`.
- Produces: `fn render(frame: &mut ratatui::Frame, deck: &Deck, st: &NavState)`

- [ ] **Step 1: Write the failing test** (append to `src/ui.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::build_deck;
    use crate::state::NavState;
    use ratatui::{backend::TestBackend, Terminal};

    const MINI: &str = r#"
    {"id":"x","result":{"type":"session_snapshot","snapshot":{
      "focused_workspace_id":"w1","focused_tab_id":"w1:t1","focused_pane_id":"w1:p1",
      "workspaces":[{"workspace_id":"w1","label":"api","number":1,"active_tab_id":"w1:t1"}],
      "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"server","number":1}],
      "panes":[{"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked"}]
    }}}"#;

    fn buffer_string(w: u16, h: u16) -> String {
        let deck = build_deck(MINI).unwrap();
        let st = NavState::new(&deck);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, &deck, &st)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_workspace_label_and_glyph() {
        let s = buffer_string(60, 20);
        assert!(s.contains("api"), "should show workspace label:\n{s}");
        assert!(s.contains("server"), "should show tab label:\n{s}");
        assert!(s.contains("◉"), "should show blocked glyph:\n{s}");
    }

    #[test]
    fn does_not_panic_on_tiny_terminal() {
        // narrow-fallback path must not crash
        let _ = buffer_string(10, 6);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib ui`
Expected: FAIL — `render` not defined.

- [ ] **Step 3: Write the implementation** (prepend to `src/ui.rs`)

```rust
use crate::model::{Counts, Deck, Workspace};
use crate::state::{workspace_rows, NavState, Row};
use crate::theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const CARD_W: u16 = 30;
const GAP: u16 = 1;

pub fn render(frame: &mut Frame, deck: &Deck, st: &NavState) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BASE)),
        area,
    );
    if deck.workspaces.is_empty() || area.width < 12 || area.height < 5 {
        frame.render_widget(
            Paragraph::new("no workspaces").style(Style::default().fg(theme::OVERLAY0)),
            area,
        );
        return;
    }

    let body = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));
    let footer = Rect::new(area.x, area.y + area.height - 1, area.width, 1);

    // horizontal scroll: keep the active card in view
    let per_screen = ((body.width + GAP) / (CARD_W + GAP)).max(1) as usize;
    let start = st.active.saturating_sub(per_screen.saturating_sub(1));

    let mut x = body.x;
    for (wi, ws) in deck.workspaces.iter().enumerate().skip(start) {
        if x + CARD_W > body.x + body.width {
            break;
        }
        let rect = Rect::new(x, body.y, CARD_W.min(body.width), body.height);
        render_card(frame, rect, ws, wi == st.active, st);
        x += CARD_W + GAP;
    }

    let hint = Line::from(vec![
        key("enter"), dim(" switch  "),
        key("←→"), dim(" ws  "),
        key("↑↓"), dim(" pane  "),
        key("b/w/i/d"), dim(" filter  "),
        key("esc"), dim(" close"),
    ]);
    frame.render_widget(Paragraph::new(hint), footer);
}

fn render_card(frame: &mut Frame, rect: Rect, ws: &Workspace, active: bool, st: &NavState) {
    let border_color = if active { theme::ACCENT } else { theme::SURFACE1 };
    let title = format!(
        " {}{} ",
        if ws.is_current { "◆ " } else { "" },
        ws.label
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default()
                .fg(if active { theme::TEXT } else { theme::OVERLAY0 })
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height == 0 {
        return;
    }

    // rollup line
    let rollup = Line::from(vec![
        rollup_span(theme::RED, "◉", ws.counts.blocked),
        Span::raw(" "),
        rollup_span(theme::YELLOW, "◍", ws.counts.working),
        Span::raw(" "),
        rollup_span(theme::TEAL, "●", ws.counts.done),
        Span::raw(" "),
        rollup_span(theme::GREEN, "✓", ws.counts.idle),
    ]);
    frame.render_widget(
        Paragraph::new(rollup),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    // rows
    let rows = workspace_rows(ws);
    let list_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    );
    let sel = st.sel.get(st.active).copied().unwrap_or(0);
    let mut lines: Vec<Line> = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        let selected = active && ri == sel;
        lines.push(render_row(ws, row, selected, st));
    }
    frame.render_widget(Paragraph::new(lines), list_area);
}

fn render_row(ws: &Workspace, row: &Row, selected: bool, st: &NavState) -> Line<'static> {
    let base = if selected {
        Style::default().bg(theme::ACCENT).fg(theme::BASE)
    } else {
        Style::default().fg(theme::TEXT)
    };
    let spans = match row {
        Row::Workspace => vec![Span::styled("  workspace", base)],
        Row::Tab(ti) => vec![Span::styled(format!(" ▸ {}", ws.tabs[*ti].label), base)],
        Row::Pane(ti, pi) => {
            let p = &ws.tabs[*ti].panes[*pi];
            let dim = st.filter.map_or(false, |f| f != p.status);
            let g = if selected {
                Style::default().fg(theme::BASE)
            } else {
                Style::default().fg(p.status.color())
            };
            let lbl = if selected {
                base
            } else if dim {
                Style::default().fg(theme::OVERLAY0)
            } else {
                Style::default().fg(theme::TEXT)
            };
            vec![
                Span::styled("   ", base),
                Span::styled(p.status.glyph().to_string(), g),
                Span::styled(format!(" {}", p.label), lbl),
            ]
        }
    };
    Line::from(spans)
}

fn rollup_span(color: ratatui::style::Color, glyph: &str, n: usize) -> Span<'static> {
    let style = if n == 0 {
        Style::default().fg(theme::OVERLAY0)
    } else {
        Style::default().fg(color)
    };
    Span::styled(format!("{glyph}{n}"), style)
}

fn key(s: &'static str) -> Span<'static> {
    Span::styled(s, Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
}
fn dim(s: &'static str) -> Span<'static> {
    Span::styled(s, Style::default().fg(theme::OVERLAY0))
}
```

Add to `src/main.rs`: `mod ui;`
Note: `Counts` import is used by nothing else here — if `cargo` warns unused, drop it from the `use`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib ui`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/ui.rs src/main.rs
git commit -m "feat: ratatui deck rendering (cards, rollup, rows, footer)"
```

---

### Task 7: Wire the binary (main.rs)

**Files:**
- Modify: `src/main.rs` (full event loop)

**Interfaces:**
- Consumes: `client`, `model::build_deck`, `state::{NavState, Outcome}`, `ui::render`.

- [ ] **Step 1: Write `src/main.rs`**

```rust
mod client;
mod model;
mod state;
mod theme;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use state::{NavState, Outcome};
use std::io::stdout;

fn main() -> Result<()> {
    let path = client::socket_path()?;
    let snapshot = client::snapshot(&path)?;
    let deck = model::build_deck(&snapshot)?;
    if deck.workspaces.is_empty() {
        eprintln!("herdr-deck: no workspaces");
        return Ok(());
    }
    let mut st = NavState::new(&deck);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut term, &deck, &mut st);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;

    // Perform the focus action AFTER restoring the terminal so the popup closes cleanly.
    if let Ok(Some(target)) = result {
        client::focus(&path, &target)?;
    }
    Ok(())
}

fn run(
    term: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    deck: &model::Deck,
    st: &mut NavState,
) -> Result<Option<state::FocusTarget>> {
    loop {
        term.draw(|f| ui::render(f, deck, st))?;
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            // Ctrl-C safety exit
            if k.code == KeyCode::Char('c')
                && k.modifiers.contains(event::KeyModifiers::CONTROL)
            {
                return Ok(None);
            }
            match st.on_key(deck, k.code) {
                Outcome::Quit => return Ok(None),
                Outcome::Focus(t) => return Ok(Some(t)),
                Outcome::Redraw => {}
            }
        }
    }
}
```

- [ ] **Step 2: Build + full test suite**

Run: `cargo build && cargo test`
Expected: compiles; all unit tests PASS.

- [ ] **Step 3: Manual smoke (outside a popup, against the live socket)**

Run: `cargo build --release && HERDR_SOCKET_PATH="$HOME/.config/herdr/herdr.sock" ./target/release/herdr-deck`
Expected: the deck renders in your terminal; `←/→ ↑/↓` move; `Enter` on a target switches your herdr focus and the program exits; `Esc` exits with no change.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire terminal event loop + focus dispatch"
```

---

### Task 8: Plugin manifest, keybinding, and real-session acceptance

**Files:**
- Create: `herdr-plugin.toml`
- Create: `open.sh`
- Create/Update: `README.md`

**Interfaces:** none (integration).

- [ ] **Step 1: Write `herdr-plugin.toml`**

```toml
id = "deck"
name = "Deck"
version = "0.1.0"
min_herdr_version = "0.7.0"
description = "Card-style workspace navigator"
platforms = ["linux", "macos"]

[[build]]
command = ["cargo", "build", "--release"]

[[actions]]
id = "open"
title = "Open deck navigator"
contexts = ["workspace"]
command = ["sh", "open.sh"]

[[panes]]
id = "picker"
title = "Deck"
placement = "popup"
width = "90%"
height = "85%"
command = ["target/release/herdr-deck"]

[[keys.command]]
key = "prefix+g"
type = "plugin_action"
command = "deck.open"
description = "card navigator"
```

- [ ] **Step 2: Write `open.sh`** (opens the popup via the portable binary path)

```sh
#!/bin/sh
exec "${HERDR_BIN_PATH:-herdr}" plugin pane open --plugin deck --entrypoint picker
```

Make it executable: `chmod +x open.sh`

- [ ] **Step 3: Link the plugin and verify registration**

```bash
cargo build --release
herdr plugin link "$PWD"
herdr plugin list
herdr plugin action list --plugin deck
```
Expected: `deck` listed; action `deck.open` present.

- [ ] **Step 4: Open the popup manually (decouples pane-open from the keybind)**

Run: `herdr plugin pane open --plugin deck --entrypoint picker`
Expected: a modal popup opens showing the card deck; arrows navigate; `Enter` switches and closes; `Esc` closes.

- [ ] **Step 5: Verify the keybinding**

In a herdr session press `Ctrl b` then `g`.
Expected: the deck popup opens.
- If the built-in navigator opens instead (binding precedence), document the conflict in README and rebind: either remap the built-in navigator off `prefix+g` in herdr config, or change the manifest key to `prefix+G`. Re-link (`herdr plugin unlink deck && herdr plugin link "$PWD"`) and retest.

- [ ] **Step 6: Acceptance with a blocked agent**

Start an agent in one workspace and drive it to a `blocked`/`working` state (or use `herdr pane report-agent <pane_id> --source test --agent demo --state blocked`). Open the deck.
Expected: that workspace's stripe/rollup shows the blocked/working glyph; it's spottable without entering the card.

- [ ] **Step 7: Write `README.md`** (install, keybind, build-from-source, the `prefix+g` caveat, screenshots) and commit

```bash
chmod +x open.sh
git add herdr-plugin.toml open.sh README.md
git commit -m "feat: plugin manifest, keybinding, and open script"
```

---

## Self-Review (completed)

**Spec coverage:** deck layout (T6), within-card nav + `←/→`/`↑/↓` (T4), status rollup + worst-stripe (T3+T6), glyphs/palette (T2), snapshot data (T3+T5), focus on Enter incl. absolute pane focus (T4+T5), search/filter — filter dimming (T4+T6); **free-text search box deferred** (see below), narrow-terminal fallback (T6), popup + keybind + `prefix+g` caveat (T8), TDD throughout.

**Deferred from spec v1 (YAGNI, tracked):**
- Free-text `/` search box — v1 ships the `b/w/i/d` state filters (higher value, already in T4/T6); text search is a fast follow (adds a query string + substring predicate over row labels).
- Live `events.subscribe` refresh — v1 renders the bootstrap snapshot.
- Windows named-pipe transport (T5 note).
- Per-pane titles (e.g. `nvim`) — snapshot has no pane title field; v1 shows `pane N`.
- Horizontal-scroll `‹/›` edge hints — T6 scrolls but omits the hint chars.

**Placeholder scan:** none — every step has concrete code/commands.

**Type consistency:** `FocusTarget` (state.rs) consumed by client.rs `focus`; `build_deck` envelope shape produced by client.rs `snapshot`; `workspace_rows`/`Row`/`NavState` shared by state.rs + ui.rs — signatures match across tasks.
