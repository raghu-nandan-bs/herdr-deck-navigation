use crate::model::{Deck, Workspace};
use crate::theme::Status;
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Browse,
    Search,
}

/// Which column has the keyboard cursor in Browse mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Rail,  // the workspace list
    Panes, // the focused workspace's tabs/panes
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
                let hay = format!(
                    "{} {} {} {} {}",
                    w.label,
                    tab.label,
                    pane.label,
                    pane.cwd.as_deref().unwrap_or(""),
                    pane.agent.as_deref().unwrap_or(""),
                )
                .to_lowercase();
                if q.is_empty() || hay.contains(&q) {
                    out.push(Loc { wi, ti, pi });
                }
            }
        }
    }
    out
}

/// Panes that want your attention, worst first: everything `blocked` (an agent is
/// waiting on you), then everything `done` (finished while you were away). `working`
/// and idle panes are deliberately absent — they don't need you.
pub fn attention_queue(deck: &Deck) -> Vec<Loc> {
    let mut out: Vec<(u8, Loc)> = Vec::new();
    for (wi, w) in deck.workspaces.iter().enumerate() {
        for (ti, tab) in w.tabs.iter().enumerate() {
            for (pi, pane) in tab.panes.iter().enumerate() {
                if matches!(pane.status, Status::Blocked | Status::Done) {
                    out.push((pane.status.rank(), Loc { wi, ti, pi }));
                }
            }
        }
    }
    out.sort_by_key(|(rank, _)| *rank); // stable: deck order preserved within a rank
    out.into_iter().map(|(_, loc)| loc).collect()
}

pub struct NavState {
    pub active: usize,     // selected workspace (the rail)
    pub sel: Vec<usize>,   // selected pane index (into workspace_panes) per workspace
    pub focus: Column,     // which column ↑/↓ moves within
    pub mode: Mode,
    pub query: String,
    pub result_sel: usize, // selected row in the search results
    /// Most-recent-first pane ids from previous sessions, for `r` (resume).
    pub recent: Vec<String>,
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
        // Open where you are. Deck used to open *on the answer* — cursor parked on
        // the first attention-queue item — but that makes Enter mean "go to whichever
        // workspace has a blocked/done agent" rather than "go where I'm pointing",
        // and nothing on screen said the cursor had been moved off you. Between two
        // workspaces sharing a folder it was invisible: same path in the detail
        // strip, different workspace on the other side of Enter. `tab` still walks
        // the queue, and the top bar still counts who needs you.
        NavState {
            active,
            sel,
            focus: Column::Rail,
            mode: Mode::Browse,
            query: String::new(),
            result_sel: 0,
            recent: Vec::new(),
        }
    }

    /// Put the cursor on `loc`, in the pane column.
    fn goto(&mut self, deck: &Deck, loc: Loc) {
        let Some(ws) = deck.workspaces.get(loc.wi) else { return };
        let Some(idx) = workspace_panes(ws)
            .iter()
            .position(|&(ti, pi)| ti == loc.ti && pi == loc.pi)
        else {
            return;
        };
        self.active = loc.wi;
        self.sel[loc.wi] = idx;
        self.focus = Column::Panes;
    }

    /// Step through the attention queue; `delta` is +1 (Tab) or -1 (Shift-Tab).
    fn cycle_attention(&mut self, deck: &Deck, delta: isize) {
        let q = attention_queue(deck);
        if q.is_empty() {
            return;
        }
        let here = self.cursor_loc(deck);
        let next = match here.and_then(|l| q.iter().position(|&x| x == l)) {
            Some(i) => (i as isize + delta).rem_euclid(q.len() as isize) as usize,
            // cursor isn't on a queue item: Tab enters at the top, Shift-Tab at the end.
            None if delta > 0 => 0,
            None => q.len() - 1,
        };
        self.goto(deck, q[next]);
    }

    /// Where the browse cursor currently sits, as a deck-wide location.
    fn cursor_loc(&self, deck: &Deck) -> Option<Loc> {
        let ws = deck.workspaces.get(self.active)?;
        let &(ti, pi) = workspace_panes(ws).get(self.sel.get(self.active).copied()?)?;
        Some(Loc {
            wi: self.active,
            ti,
            pi,
        })
    }

    /// The pane `r` jumps back to: the most recent one that still exists and isn't
    /// the pane you came from.
    fn resume_target(&self, deck: &Deck) -> Option<String> {
        self.recent.iter().find_map(|id| {
            deck.workspaces
                .iter()
                .flat_map(|w| w.tabs.iter())
                .flat_map(|t| t.panes.iter())
                .find(|p| &p.id == id && !p.is_current)
                .map(|p| p.id.clone())
        })
    }

    fn clamp_sel(&mut self, deck: &Deck) {
        let n = workspace_panes(&deck.workspaces[self.active]).len();
        let s = self.sel[self.active];
        self.sel[self.active] = if n == 0 { 0 } else { s.min(n - 1) };
    }

    /// Keep the cursor valid after a live refresh (workspaces added/removed, panes
    /// closed). Renames leave ids/order unchanged, so the cursor stays put.
    pub fn reconcile(&mut self, deck: &Deck) {
        let n = deck.workspaces.len();
        if n == 0 {
            self.active = 0;
            self.sel.clear();
            return;
        }
        self.active = self.active.min(n - 1);
        self.sel.resize(n, 0);
        for (i, ws) in deck.workspaces.iter().enumerate() {
            let pc = workspace_panes(ws).len();
            self.sel[i] = if pc == 0 { 0 } else { self.sel[i].min(pc - 1) };
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
            // number keys are a shortcut straight to a workspace
            KeyCode::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if i < deck.workspaces.len() {
                    self.active = i;
                    self.clamp_sel(deck);
                }
                Outcome::Redraw
            }
            // ← / → switch which column the cursor is in
            KeyCode::Left | KeyCode::Char('h') => {
                self.focus = Column::Rail;
                Outcome::Redraw
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.focus = Column::Panes;
                Outcome::Redraw
            }
            // ↑ / ↓ move within the focused column
            KeyCode::Down | KeyCode::Char('j') => {
                match self.focus {
                    Column::Rail => {
                        self.active =
                            (self.active + 1).min(deck.workspaces.len().saturating_sub(1));
                        self.clamp_sel(deck);
                    }
                    Column::Panes => {
                        let n = workspace_panes(&deck.workspaces[self.active]).len();
                        if n > 0 {
                            self.sel[self.active] = (self.sel[self.active] + 1).min(n - 1);
                        }
                    }
                }
                Outcome::Redraw
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match self.focus {
                    Column::Rail => {
                        self.active = self.active.saturating_sub(1);
                        self.clamp_sel(deck);
                    }
                    Column::Panes => {
                        self.sel[self.active] = self.sel[self.active].saturating_sub(1);
                    }
                }
                Outcome::Redraw
            }
            // Tab walks the attention queue — the whole point of the tool.
            KeyCode::Tab => {
                self.cycle_attention(deck, 1);
                Outcome::Redraw
            }
            KeyCode::BackTab => {
                self.cycle_attention(deck, -1);
                Outcome::Redraw
            }
            // r resumes: alt-tab back to the pane you were in before this one.
            KeyCode::Char('r') => match self.resume_target(deck) {
                Some(id) => Outcome::Focus(FocusTarget::Pane(id)),
                None => Outcome::Redraw,
            },
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
        // On the rail, Enter switches to the whole workspace; in the pane column,
        // Enter switches to the selected pane.
        if self.focus == Column::Rail {
            return Some(FocusTarget::Workspace(w.id.clone()));
        }
        let panes = workspace_panes(w);
        match panes.get(self.sel.get(self.active).copied().unwrap_or(0)) {
            Some(&(ti, pi)) => Some(FocusTarget::Pane(w.tabs[ti].panes[pi].id.clone())),
            None => Some(FocusTarget::Workspace(w.id.clone())),
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
    fn opens_where_you_are_even_though_another_workspace_needs_you() {
        // w2 is the focused workspace and infra's w3:p1 is blocked. The cursor must
        // stay on w2: Enter means "go where I'm pointing", and moving the cursor off
        // you silently turns a reflex Enter into a jump to a workspace you never
        // chose — invisible when the two share a folder. `tab` is how you reach the
        // queue.
        let st = NavState::new(&deck());
        assert_eq!(st.active, 1); // w2, where we are — not w3, which needs us
        assert_eq!(st.focus, Column::Rail);
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
    fn left_right_switch_column_vertical_moves_within() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.active = 0;
        st.sel[0] = 0;
        st.focus = Column::Rail;
        // on the rail: ↓ moves workspace
        st.on_key(&d, KeyCode::Down);
        assert_eq!(st.active, 1);
        // → switches to the pane column: ↓ now moves panes
        st.on_key(&d, KeyCode::Right);
        assert_eq!(st.focus, Column::Panes);
        st.active = 0; // api has 2 panes
        st.sel[0] = 0;
        st.on_key(&d, KeyCode::Down);
        assert_eq!(st.sel[0], 1);
        st.on_key(&d, KeyCode::Down); // clamp
        assert_eq!(st.sel[0], 1);
        // ← back to the rail
        st.on_key(&d, KeyCode::Left);
        assert_eq!(st.focus, Column::Rail);
    }

    #[test]
    fn enter_on_pane_column_focuses_pane_on_rail_focuses_workspace() {
        let d = deck();
        let mut st = NavState::new(&d);
        st.active = 0;
        st.focus = Column::Rail;
        // rail focus → workspace target
        match st.on_key(&d, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Workspace(id)) => assert_eq!(id, "w1"),
            other => panic!("expected workspace focus, got {other:?}"),
        }
        // pane column → pane target
        st.focus = Column::Panes;
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
    fn reconcile_clamps_cursor_after_workspaces_shrink() {
        let big = deck(); // 3 workspaces
        let mut st = NavState::new(&big);
        st.active = 2;
        st.sel = vec![9, 9, 9];
        let small = build_deck(
            r#"{"result":{"snapshot":{
              "workspaces":[{"workspace_id":"w1","label":"api","number":1}],
              "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
              "panes":[{"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle"}]
            }}}"#,
            &Context::default(),
        )
        .unwrap();
        st.reconcile(&small);
        assert_eq!(st.active, 0);
        assert_eq!(st.sel.len(), 1);
        assert_eq!(st.sel[0], 0); // clamped to the single pane
    }

    #[test]
    fn esc_in_browse_quits() {
        let d = deck();
        let mut st = NavState::new(&d);
        assert!(matches!(st.on_key(&d, KeyCode::Esc), Outcome::Quit));
    }

    /// Two workspaces with a mix of statuses, used for attention-queue ordering.
    const TRIAGE: &str = r#"
    {"result":{"snapshot":{
      "focused_workspace_id":"w1","focused_pane_id":"w1:p1",
      "workspaces":[
        {"workspace_id":"w1","label":"api","number":1},
        {"workspace_id":"w2","label":"web","number":2}
      ],
      "tabs":[
        {"tab_id":"w1:t1","workspace_id":"w1","label":"server","number":1},
        {"tab_id":"w2:t1","workspace_id":"w2","label":"ui","number":1}
      ],
      "panes":[
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle","label":"shell"},
        {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"done","label":"migrate"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"blocked","label":"review"},
        {"pane_id":"w2:p2","tab_id":"w2:t1","workspace_id":"w2","agent_status":"working","label":"build"}
      ]
    }}}"#;

    fn triage() -> Deck {
        build_deck(TRIAGE, &Context::default()).unwrap()
    }

    #[test]
    fn attention_queue_lists_blocked_before_done_and_skips_the_rest() {
        let q = attention_queue(&triage());
        // blocked (w2:p1) first, then done (w1:p2); working and idle are not "needs you"
        assert_eq!(q, vec![Loc { wi: 1, ti: 0, pi: 0 }, Loc { wi: 0, ti: 0, pi: 1 }]);
    }

    #[test]
    fn attention_queue_is_empty_when_nothing_needs_you() {
        let d = build_deck(
            r#"{"result":{"snapshot":{
              "workspaces":[{"workspace_id":"w1","label":"api","number":1}],
              "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
              "panes":[{"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle"}]
            }}}"#,
            &Context::default(),
        )
        .unwrap();
        assert!(attention_queue(&d).is_empty());
    }

    #[test]
    fn opens_on_the_current_workspace_and_its_current_pane() {
        // w1:p1 is where we are; w2:p1 is blocked. Open on w1, cursor on w1:p1.
        let d = triage();
        let st = NavState::new(&d);
        assert_eq!(st.active, 0);
        assert_eq!(st.sel[0], 0);
        assert_eq!(st.focus, Column::Rail);
    }

    #[test]
    fn enter_on_open_goes_nowhere_new_rather_than_jumping_to_the_queue() {
        // The regression this guards: opening Deck from one workspace and pressing
        // Enter used to land you in whichever workspace had a blocked/done agent.
        let d = triage(); // w1 is current; w2:p1 is blocked
        let mut st = NavState::new(&d);
        match st.on_key(&d, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Workspace(id)) => assert_eq!(id, "w1"),
            other => panic!("expected the workspace we came from, got {other:?}"),
        }
    }

    #[test]
    fn opens_on_the_current_workspace_when_nothing_needs_you() {
        let d = build_deck(
            r#"{"result":{"snapshot":{
              "focused_workspace_id":"w2","focused_pane_id":"w2:p1",
              "workspaces":[
                {"workspace_id":"w1","label":"api","number":1},
                {"workspace_id":"w2","label":"web","number":2}
              ],
              "tabs":[
                {"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1},
                {"tab_id":"w2:t1","workspace_id":"w2","label":"t","number":1}
              ],
              "panes":[
                {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle"},
                {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"idle"}
              ]
            }}}"#,
            &Context::default(),
        )
        .unwrap();
        let st = NavState::new(&d);
        assert_eq!(st.active, 1);
        assert_eq!(st.focus, Column::Rail);
    }

    #[test]
    fn tab_cycles_forward_through_the_attention_queue_and_wraps() {
        let d = triage();
        let mut st = NavState::new(&d);
        // opens where we are (w1:p1), off the queue
        assert_eq!((st.active, st.sel[st.active]), (0, 0));
        st.on_key(&d, KeyCode::Tab); // → blocked pane (queue slot 0)
        assert_eq!((st.active, st.sel[st.active]), (1, 0));
        st.on_key(&d, KeyCode::Tab); // → done pane in w1
        assert_eq!((st.active, st.sel[st.active]), (0, 1));
        st.on_key(&d, KeyCode::Tab); // wraps back to blocked
        assert_eq!((st.active, st.sel[st.active]), (1, 0));
    }

    #[test]
    fn tab_moves_into_the_pane_column_so_enter_goes_to_that_pane() {
        let d = triage();
        let mut st = NavState::new(&d);
        assert_eq!(st.focus, Column::Rail);
        st.on_key(&d, KeyCode::Tab);
        assert_eq!(st.focus, Column::Panes);
        match st.on_key(&d, KeyCode::Enter) {
            Outcome::Focus(FocusTarget::Pane(id)) => assert_eq!(id, "w2:p1"),
            other => panic!("expected the blocked pane, got {other:?}"),
        }
    }

    #[test]
    fn shift_tab_cycles_backward_through_the_attention_queue() {
        let d = triage();
        let mut st = NavState::new(&d);
        st.on_key(&d, KeyCode::BackTab); // from slot 0 wraps to the last slot
        assert_eq!((st.active, st.sel[st.active]), (0, 1));
    }

    #[test]
    fn tab_is_inert_when_nothing_needs_you() {
        let d = deck(); // MINI: w3:p1 is the only attention item
        let mut st = NavState::new(&d);
        st.active = 0;
        st.focus = Column::Rail;
        let calm = build_deck(
            r#"{"result":{"snapshot":{
              "workspaces":[{"workspace_id":"w1","label":"api","number":1}],
              "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
              "panes":[{"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle"}]
            }}}"#,
            &Context::default(),
        )
        .unwrap();
        let mut st2 = NavState::new(&calm);
        st2.on_key(&calm, KeyCode::Tab);
        assert_eq!(st2.active, 0);
        assert_eq!(st2.focus, Column::Rail);
        let _ = st.active;
    }

    #[test]
    fn search_matches_cwd_and_agent_not_just_labels() {
        let d = build_deck(
            r#"{"result":{"snapshot":{
              "workspaces":[{"workspace_id":"w1","label":"api","number":1}],
              "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
              "panes":[
                {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle",
                 "label":"one","cwd":"/home/me/infra","agent":"claude"},
                {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle",
                 "label":"two","cwd":"/home/me/web","agent":"codex"}
              ]
            }}}"#,
            &Context::default(),
        )
        .unwrap();
        assert_eq!(search_results(&d, "infra"), vec![Loc { wi: 0, ti: 0, pi: 0 }]);
        assert_eq!(search_results(&d, "codex"), vec![Loc { wi: 0, ti: 0, pi: 1 }]);
    }

    #[test]
    fn r_resumes_the_most_recent_pane_that_is_not_the_one_you_came_from() {
        let d = triage(); // w1:p1 is_current (focused_pane_id)
        let mut st = NavState::new(&d);
        // most-recent-first; w1:p1 is where we are, w9:p9 no longer exists
        st.recent = vec!["w1:p1".into(), "w9:p9".into(), "w2:p2".into()];
        match st.on_key(&d, KeyCode::Char('r')) {
            Outcome::Focus(FocusTarget::Pane(id)) => assert_eq!(id, "w2:p2"),
            other => panic!("expected resume focus, got {other:?}"),
        }
    }

    #[test]
    fn r_does_nothing_without_a_resumable_pane() {
        let d = triage();
        let mut st = NavState::new(&d);
        st.recent = vec!["w1:p1".into()]; // only the pane we came from
        assert!(matches!(st.on_key(&d, KeyCode::Char('r')), Outcome::Redraw));
    }
}
