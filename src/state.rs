use crate::model::{Deck, Workspace};
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Browse,
    Search,
}

#[derive(Debug)]
pub enum FocusTarget {
    Workspace(String),
    Pane(String),
}

#[derive(Debug)]
pub enum Outcome {
    Redraw,
    Focus(FocusTarget),
    Quit,
}

/// A pane located within the whole deck (workspace / tab / pane indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    pub wi: usize,
    pub ti: usize,
    pub pi: usize,
}

/// Flat (tab_idx, pane_idx) list of a workspace's panes, in display order.
pub fn workspace_panes(ws: &Workspace) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for (ti, tab) in ws.tabs.iter().enumerate() {
        for pi in 0..tab.panes.len() {
            v.push((ti, pi));
        }
    }
    v
}

/// Every pane across the deck matching `query` (substring over workspace/tab/pane
/// labels). Empty query returns all panes.
pub fn search_results(deck: &Deck, query: &str) -> Vec<Loc> {
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    for (wi, w) in deck.workspaces.iter().enumerate() {
        for (ti, tab) in w.tabs.iter().enumerate() {
            for (pi, pane) in tab.panes.iter().enumerate() {
                let hay = format!("{} {} {}", w.label, tab.label, pane.label).to_lowercase();
                if q.is_empty() || hay.contains(&q) {
                    out.push(Loc { wi, ti, pi });
                }
            }
        }
    }
    out
}

pub struct NavState {
    pub active: usize,     // selected workspace (the rail)
    pub sel: Vec<usize>,   // selected pane index (into workspace_panes) per workspace
    pub mode: Mode,
    pub query: String,
    pub result_sel: usize, // selected row in the search results
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
                workspace_panes(w)
                    .iter()
                    .position(|&(ti, pi)| w.tabs[ti].panes[pi].is_current)
                    .unwrap_or(0)
            })
            .collect();
        NavState {
            active,
            sel,
            mode: Mode::Browse,
            query: String::new(),
            result_sel: 0,
        }
    }

    pub fn on_key(&mut self, deck: &Deck, code: KeyCode) -> Outcome {
        match self.mode {
            Mode::Browse => self.on_browse_key(deck, code),
            Mode::Search => self.on_search_key(deck, code),
        }
    }

    fn on_browse_key(&mut self, deck: &Deck, code: KeyCode) -> Outcome {
        match code {
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.query.clear();
                self.result_sel = 0;
                Outcome::Redraw
            }
            KeyCode::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if i < deck.workspaces.len() {
                    self.active = i;
                }
                Outcome::Redraw
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.active = self.active.saturating_sub(1);
                Outcome::Redraw
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.active = (self.active + 1).min(deck.workspaces.len().saturating_sub(1));
                Outcome::Redraw
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = workspace_panes(&deck.workspaces[self.active]).len();
                if n > 0 {
                    self.sel[self.active] = (self.sel[self.active] + 1).min(n - 1);
                }
                Outcome::Redraw
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel[self.active] = self.sel[self.active].saturating_sub(1);
                Outcome::Redraw
            }
            KeyCode::Enter => match self.browse_target(deck) {
                Some(t) => Outcome::Focus(t),
                None => Outcome::Redraw,
            },
            KeyCode::Esc => Outcome::Quit,
            _ => Outcome::Redraw,
        }
    }

    fn browse_target(&self, deck: &Deck) -> Option<FocusTarget> {
        let w = deck.workspaces.get(self.active)?;
        let panes = workspace_panes(w);
        match panes.get(self.sel.get(self.active).copied().unwrap_or(0)) {
            Some(&(ti, pi)) => Some(FocusTarget::Pane(w.tabs[ti].panes[pi].id.clone())),
            None => Some(FocusTarget::Workspace(w.id.clone())), // empty workspace
        }
    }

    fn on_search_key(&mut self, deck: &Deck, code: KeyCode) -> Outcome {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                self.query.clear();
                Outcome::Redraw
            }
            KeyCode::Enter => {
                let hits = search_results(deck, &self.query);
                match hits.get(self.result_sel) {
                    Some(loc) => {
                        let id = deck.workspaces[loc.wi].tabs[loc.ti].panes[loc.pi].id.clone();
                        Outcome::Focus(FocusTarget::Pane(id))
                    }
                    None => Outcome::Redraw,
                }
            }
            KeyCode::Down => {
                let n = search_results(deck, &self.query).len();
                if n > 0 {
                    self.result_sel = (self.result_sel + 1).min(n - 1);
                }
                Outcome::Redraw
            }
            KeyCode::Up => {
                self.result_sel = self.result_sel.saturating_sub(1);
                Outcome::Redraw
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.result_sel = 0;
                Outcome::Redraw
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.result_sel = 0;
                Outcome::Redraw
            }
            _ => Outcome::Redraw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{build_deck, Context};
    use crossterm::event::KeyCode;

    const MINI: &str = r#"
    {"id":"x","result":{"type":"session_snapshot","snapshot":{
      "focused_workspace_id":"w2","focused_pane_id":"w2:p1",
      "workspaces":[
        {"workspace_id":"w1","label":"api","number":1},
        {"workspace_id":"w2","label":"web","number":2},
        {"workspace_id":"w3","label":"infra","number":3}
      ],
      "tabs":[
        {"tab_id":"w1:t1","workspace_id":"w1","label":"server","number":1},
        {"tab_id":"w2:t1","workspace_id":"w2","label":"ui","number":1},
        {"tab_id":"w3:t1","workspace_id":"w3","label":"shell","number":1}
      ],
      "panes":[
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle","label":"loadtest agent"},
        {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"working"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"working"},
        {"pane_id":"w3:p1","tab_id":"w3:t1","workspace_id":"w3","agent_status":"blocked"}
      ]
    }}}"#;

    fn deck() -> Deck {
        build_deck(MINI, &Context::default()).unwrap()
    }

    #[test]
    fn starts_on_current_workspace() {
        let st = NavState::new(&deck());
        assert_eq!(st.active, 1); // w2 focused
    }

    #[test]
    fn number_key_jumps_workspace() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.on_key(&d, KeyCode::Char('3'));
        assert_eq!(st.active, 2);
        st.on_key(&d, KeyCode::Char('1'));
        assert_eq!(st.active, 0);
        st.on_key(&d, KeyCode::Char('9')); // out of range, ignored
        assert_eq!(st.active, 0);
    }

    #[test]
    fn down_moves_through_panes_clamped() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.active = 0; // api has 2 panes
        st.sel[0] = 0;
        st.on_key(&d, KeyCode::Down);
        assert_eq!(st.sel[0], 1);
        st.on_key(&d, KeyCode::Down); // clamp
        assert_eq!(st.sel[0], 1);
        st.on_key(&d, KeyCode::Up);
        assert_eq!(st.sel[0], 0);
    }

    #[test]
    fn enter_focuses_selected_pane() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.active = 0;
        st.sel[0] = 1; // w1:p2
        match st.on_key(&d, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Pane(id)) => assert_eq!(id, "w1:p2"),
            other => panic!("expected pane focus, got {other:?}"),
        }
    }

    #[test]
    fn slash_enters_search_and_typing_filters() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.on_key(&d, KeyCode::Char('/'));
        assert_eq!(st.mode, Mode::Search);
        for c in "loadtest".chars() {
            st.on_key(&d, KeyCode::Char(c));
        }
        let hits = search_results(&d, &st.query);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0], Loc { wi: 0, ti: 0, pi: 0 });
    }

    #[test]
    fn search_enter_focuses_the_hit_then_esc_exits() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.on_key(&d, KeyCode::Char('/'));
        for c in "blocked".chars() {
            st.on_key(&d, KeyCode::Char(c));
        }
        // "blocked" matches no label text; broaden to something that hits infra's pane
        st.query.clear();
        for c in "infra".chars() {
            st.on_key(&d, KeyCode::Char(c));
        }
        match st.on_key(&d, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Pane(id)) => assert_eq!(id, "w3:p1"),
            other => panic!("expected pane focus, got {other:?}"),
        }
        // re-enter search then esc returns to browse
        st.on_key(&d, KeyCode::Char('/'));
        st.on_key(&d, KeyCode::Esc);
        assert_eq!(st.mode, Mode::Browse);
    }

    #[test]
    fn esc_in_browse_quits() {
        let d = deck();
        let mut st = NavState::new(&d);
        assert!(matches!(st.on_key(&d, KeyCode::Esc), Outcome::Quit));
    }
}
