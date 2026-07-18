use crate::model::Deck;
use crate::state::{search_results, workspace_panes, Mode, NavState};
use crate::theme::Palette;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const RAIL_W: u16 = 28;

pub fn render(frame: &mut Frame, deck: &Deck, st: &NavState, p: &Palette) {
    let area = frame.area();
    if deck.workspaces.is_empty() || area.width < 20 || area.height < 6 {
        frame.render_widget(
            Paragraph::new("no workspaces").style(Style::default().fg(p.overlay0)),
            area,
        );
        return;
    }

    let v = Layout::vertical([
        Constraint::Length(1), // search / hint bar
        Constraint::Length(1), // divider
        Constraint::Min(0),    // body
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_topbar(frame, v[0], deck, st, p);
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(p.surface1)),
        v[1],
    );

    match st.mode {
        Mode::Browse => {
            let h = Layout::horizontal([Constraint::Length(RAIL_W), Constraint::Min(0)]).split(v[2]);
            render_rail(frame, h[0], deck, st, p);
            render_focus(frame, h[1], deck, st, p);
        }
        Mode::Search => render_results(frame, v[2], deck, st, p),
    }

    render_footer(frame, v[3], st, p);
}

fn render_topbar(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let total: usize = deck
        .workspaces
        .iter()
        .flat_map(|w| w.tabs.iter())
        .map(|t| t.panes.len())
        .sum();
    let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(12)]).split(area);

    let mut left = vec![Span::styled(
        " / ",
        Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
    )];
    match st.mode {
        Mode::Search if !st.query.is_empty() => {
            left.push(Span::styled(st.query.clone(), Style::default().fg(p.text)));
            left.push(Span::styled("▏", Style::default().fg(p.accent)));
        }
        Mode::Search => left.push(Span::styled(
            "search panes…",
            Style::default().fg(p.overlay0),
        )),
        Mode::Browse => left.push(Span::styled(
            "press / to search · 1–9 to jump",
            Style::default().fg(p.overlay0),
        )),
    }
    frame.render_widget(Paragraph::new(Line::from(left)), cols[0]);
    frame.render_widget(
        Paragraph::new(format!("{total} panes "))
            .alignment(Alignment::Right)
            .style(Style::default().fg(p.overlay0)),
        cols[1],
    );
}

fn render_rail(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(p.surface1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut lines = vec![Line::from(Span::styled(
        " WORKSPACES",
        Style::default().fg(p.overlay0),
    ))];
    for (i, w) in deck.workspaces.iter().enumerate() {
        let active = i == st.active;
        let marker = if active { "▎" } else { " " };
        let num_style = if active {
            Style::default().fg(p.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.overlay0)
        };
        let name_style = if active {
            Style::default().fg(p.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.subtext0)
        };
        let num = if i < 9 {
            format!("{} ", i + 1)
        } else {
            "  ".to_string()
        };
        let pc = workspace_panes(w).len();
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(p.accent)),
            Span::styled(num, num_style),
            Span::styled("● ", Style::default().fg(w.worst.color(p))),
            Span::styled(truncate(&w.label, inner.width.saturating_sub(9) as usize), name_style),
            Span::styled(format!("  {pc}"), Style::default().fg(p.overlay0)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_focus(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let w = &deck.workspaces[st.active];
    if area.height < 2 {
        return;
    }
    // header: workspace name + rollup
    let head = Layout::horizontal([Constraint::Min(0), Constraint::Length(20)])
        .split(Rect::new(area.x + 1, area.y, area.width.saturating_sub(1), 1));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            w.label.clone(),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ))),
        head[0],
    );
    let c = &w.counts;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            rollup(p.red, "◉", c.blocked, p),
            Span::raw(" "),
            rollup(p.yellow, "◍", c.working, p),
            Span::raw(" "),
            rollup(p.teal, "●", c.done, p),
            Span::raw(" "),
            rollup(p.green, "✓", c.idle, p),
        ]))
        .alignment(Alignment::Right),
        head[1],
    );

    // body: tabs + panes, selected pane highlighted
    let sel = st.sel.get(st.active).copied().unwrap_or(0);
    let mut lines: Vec<Line> = Vec::new();
    let mut sel_line = 0usize;
    let mut pane_idx = 0usize;
    for tab in &w.tabs {
        lines.push(Line::from(vec![
            Span::styled(" ▸ ", Style::default().fg(p.overlay0)),
            Span::styled(
                tab.label.clone(),
                Style::default().fg(p.subtext0).add_modifier(Modifier::BOLD),
            ),
        ]));
        for pane in &tab.panes {
            let selected = pane_idx == sel;
            if selected {
                sel_line = lines.len();
            }
            let (marker, gstyle, lstyle) = if selected {
                (
                    Span::styled("▎", Style::default().fg(p.accent)),
                    Style::default().fg(pane.status.color(p)),
                    Style::default().fg(p.text).add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    Span::raw(" "),
                    Style::default().fg(pane.status.color(p)),
                    Style::default().fg(p.subtext0),
                )
            };
            lines.push(Line::from(vec![
                marker,
                Span::styled(format!("  {} ", pane.status.glyph()), gstyle),
                Span::styled(pane.label.clone(), lstyle),
            ]));
            pane_idx += 1;
        }
    }
    let body = Rect::new(area.x + 1, area.y + 2, area.width.saturating_sub(1), area.height.saturating_sub(2));
    let offset = scroll_offset(sel_line, body.height as usize, lines.len());
    frame.render_widget(Paragraph::new(lines).scroll((offset as u16, 0)), body);
}

fn render_results(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let hits = search_results(deck, &st.query);
    if hits.is_empty() {
        frame.render_widget(
            Paragraph::new("  no matches").style(Style::default().fg(p.overlay0)),
            area,
        );
        return;
    }
    let lines: Vec<Line> = hits
        .iter()
        .enumerate()
        .map(|(i, loc)| {
            let w = &deck.workspaces[loc.wi];
            let tab = &w.tabs[loc.ti];
            let pane = &tab.panes[loc.pi];
            let selected = i == st.result_sel;
            let marker = if selected {
                Span::styled("▎", Style::default().fg(p.accent))
            } else {
                Span::raw(" ")
            };
            let lstyle = if selected {
                Style::default().fg(p.text).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(p.subtext0)
            };
            Line::from(vec![
                marker,
                Span::styled(
                    format!("  {} ", pane.status.glyph()),
                    Style::default().fg(pane.status.color(p)),
                ),
                Span::styled(format!("{:<22}", truncate(&pane.label, 22)), lstyle),
                Span::styled(
                    format!("  {} ▸ {}", w.label, tab.label),
                    Style::default().fg(p.overlay0),
                ),
            ])
        })
        .collect();
    let offset = scroll_offset(st.result_sel, area.height as usize, lines.len());
    frame.render_widget(Paragraph::new(lines).scroll((offset as u16, 0)), area);
}

fn render_footer(frame: &mut Frame, area: Rect, st: &NavState, p: &Palette) {
    let line = match st.mode {
        Mode::Browse => Line::from(vec![
            key("1–9", p), dim(" workspace   ", p),
            key("↑ ↓", p), dim(" pane   ", p),
            key("/", p), dim(" search   ", p),
            key("↵", p), dim(" switch   ", p),
            key("esc", p), dim(" close", p),
        ]),
        Mode::Search => Line::from(vec![
            dim("type to filter   ", p),
            key("↑ ↓", p), dim(" select   ", p),
            key("↵", p), dim(" switch   ", p),
            key("esc", p), dim(" back", p),
        ]),
    };
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}

fn scroll_offset(sel: usize, height: usize, len: usize) -> usize {
    if height == 0 || len <= height {
        return 0;
    }
    if sel >= height {
        (sel + 1 - height).min(len - height)
    } else {
        0
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn rollup(color: ratatui::style::Color, glyph: &str, n: usize, p: &Palette) -> Span<'static> {
    let style = if n == 0 {
        Style::default().fg(p.overlay0)
    } else {
        Style::default().fg(color)
    };
    Span::styled(format!("{glyph}{n}"), style)
}

fn key(s: &'static str, p: &Palette) -> Span<'static> {
    Span::styled(s, Style::default().fg(p.accent).add_modifier(Modifier::BOLD))
}
fn dim(s: &'static str, p: &Palette) -> Span<'static> {
    Span::styled(s, Style::default().fg(p.overlay0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{build_deck, Context};
    use crate::state::NavState;
    use crate::theme::Palette;
    use ratatui::{backend::TestBackend, Terminal};

    const MINI: &str = r#"
    {"id":"x","result":{"type":"session_snapshot","snapshot":{
      "focused_workspace_id":"w1","focused_pane_id":"w1:p1",
      "workspaces":[
        {"workspace_id":"w1","label":"esd","number":1},
        {"workspace_id":"w2","label":"load-generator","number":2}
      ],
      "tabs":[
        {"tab_id":"w1:t1","workspace_id":"w1","label":"server","number":1},
        {"tab_id":"w2:t1","workspace_id":"w2","label":"lg-runner","number":1}
      ],
      "panes":[
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked","label":"pane 1"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"idle","label":"loadtest agent"}
      ]
    }}}"#;

    fn draw(setup: impl FnOnce(&mut NavState), w: u16, h: u16) -> String {
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let mut st = NavState::new(&deck);
        setup(&mut st);
        let pal = Palette::catppuccin();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, &deck, &st, &pal)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf.cell((x, y)).unwrap().symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rail_lists_all_workspaces_focus_shows_active() {
        let s = draw(|st| st.active = 0, 80, 16);
        assert!(s.contains("WORKSPACES"), "rail header:\n{s}");
        assert!(s.contains("esd"), "rail + focus show esd:\n{s}");
        assert!(s.contains("load-generator"), "rail lists other ws:\n{s}");
        assert!(s.contains("pane 1"), "focus shows active ws panes:\n{s}");
    }

    #[test]
    fn search_mode_shows_filtered_results() {
        let s = draw(
            |st| {
                st.mode = crate::state::Mode::Search;
                st.query = "loadtest".into();
            },
            80,
            16,
        );
        assert!(s.contains("loadtest agent"), "result label:\n{s}");
        assert!(s.contains("load-generator ▸ lg-runner"), "result path:\n{s}");
    }

    #[test]
    fn does_not_panic_on_tiny_terminal() {
        let _ = draw(|_| {}, 20, 6);
        let _ = draw(|st| st.active = 1, 24, 8);
    }
}
