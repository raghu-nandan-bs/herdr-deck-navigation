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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{build_deck, Context};
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
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let st = NavState::new(&deck);
        assert_eq!(st.active, 1); // w2 is focused
    }

    #[test]
    fn rows_start_with_workspace_then_tabs_and_panes() {
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let rows = workspace_rows(&deck.workspaces[0]);
        assert_eq!(rows[0], Row::Workspace);
        assert_eq!(rows[1], Row::Tab(0));
        assert_eq!(rows[2], Row::Pane(0, 0));
    }

    #[test]
    fn left_right_move_between_cards_clamped() {
        let deck = build_deck(MINI, &Context::default()).unwrap();
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
    fn right_past_last_workspace_clamps_active() {
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let mut st = NavState::new(&deck);
        st.active = 1; // last workspace (w2)
        st.on_key(&deck, KeyCode::Right); // clamp at len-1 == 1
        assert_eq!(st.active, 1);
        st.on_key(&deck, KeyCode::Right); // still clamped
        assert_eq!(st.active, 1);
    }

    #[test]
    fn up_down_move_within_active_card_clamped() {
        let deck = build_deck(MINI, &Context::default()).unwrap();
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
        let deck = build_deck(MINI, &Context::default()).unwrap();
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
        let deck = build_deck(MINI, &Context::default()).unwrap();
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
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let mut st = NavState::new(&deck);
        assert!(matches!(st.on_key(&deck, KeyCode::Esc), Outcome::Quit));
    }
}
