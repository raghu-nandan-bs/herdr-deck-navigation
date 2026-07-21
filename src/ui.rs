use crate::model::{Deck, Pane};
use crate::state::{search_results, workspace_panes, Column, Mode, NavState};
use crate::theme::Palette;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Rail width: wide enough for the longest workspace name (plus number, dot, and
/// pane count), capped so the focus pane keeps room.
fn rail_width(deck: &Deck, area_w: u16) -> u16 {
    let name_w = deck
        .workspaces
        .iter()
        .map(|w| w.label.chars().count())
        .max()
        .unwrap_or(8) as u16;
    // chrome ≈ bar + " N " + "● " + name + "  NN"
    (name_w + 11).clamp(22, (area_w * 2 / 5).max(22).min(54))
}

pub fn render(frame: &mut Frame, deck: &Deck, st: &NavState, p: &Palette) {
    let area = frame.area();
    // Solid theme background — readability first. A transparent panel is unreadable
    // in a dark theme over a dark wallpaper, so we fill with the theme's panel bg.
    frame.render_widget(Block::default().style(Style::default().bg(p.panel_bg)), area);
    if deck.workspaces.is_empty() || area.width < 24 || area.height < 8 {
        frame.render_widget(
            Paragraph::new("no workspaces").style(Style::default().fg(p.overlay0).bg(p.panel_bg)),
            area,
        );
        return;
    }

    let v = Layout::vertical([
        Constraint::Length(1), // search bar
        Constraint::Length(1), // divider
        Constraint::Min(0),    // body
        Constraint::Length(1), // divider
        Constraint::Length(1), // detail strip
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_topbar(frame, v[0], deck, st, p);
    hrule(frame, v[1], p);

    match st.mode {
        Mode::Browse => {
            let rw = rail_width(deck, area.width);
            let h = Layout::horizontal([Constraint::Length(rw), Constraint::Min(0)]).split(v[2]);
            render_rail(frame, h[0], deck, st, p);
            render_focus(frame, h[1], deck, st, p);
        }
        Mode::Search => render_results(frame, v[2], deck, st, p),
    }

    hrule(frame, v[3], p);
    render_detail(frame, v[4], deck, st, p);
    render_footer(frame, v[5], st, p);
}

fn hrule(frame: &mut Frame, area: Rect, p: &Palette) {
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(p.surface1)),
        area,
    );
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
        "  / ",
        Style::default().fg(p.accent).add_modifier(Modifier::BOLD),
    )];
    match st.mode {
        Mode::Search if !st.query.is_empty() => {
            left.push(Span::styled(st.query.clone(), Style::default().fg(p.text)));
            left.push(Span::styled("▏", Style::default().fg(p.accent)));
        }
        Mode::Search => left.push(Span::styled("search panes…", Style::default().fg(p.overlay0))),
        Mode::Browse => left.push(Span::styled(
            "press / to search · 1–9 to jump",
            Style::default().fg(p.overlay0),
        )),
    }
    frame.render_widget(Paragraph::new(Line::from(left)), cols[0]);
    frame.render_widget(
        Paragraph::new(format!("{total} panes  "))
            .alignment(Alignment::Right)
            .style(Style::default().fg(p.overlay0)),
        cols[1],
    );
}

fn section_header(text: &str, p: &Palette) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {}", text),
        Style::default().fg(p.overlay0).add_modifier(Modifier::BOLD),
    ))
}

fn render_rail(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(p.surface1));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 {
        return;
    }
    let focused = st.focus == Column::Rail;
    let w = inner.width as usize;

    // header + blank line, then rows
    frame.render_widget(
        Paragraph::new(section_header("WORKSPACES", p)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    let list = Rect::new(inner.x, inner.y + 2, inner.width, inner.height.saturating_sub(2));

    let mut lines = Vec::new();
    for (i, ws) in deck.workspaces.iter().enumerate() {
        let active = i == st.active;
        let bg = if active {
            Style::default().bg(p.surface0)
        } else {
            Style::default()
        };
        let (bar, bar_c) = match (active, focused) {
            (true, true) => ("▌", p.accent),
            (true, false) => ("▎", p.overlay0),
            (false, _) => (" ", p.overlay0),
        };
        let num = if i < 9 {
            format!("{} ", i + 1)
        } else {
            "  ".to_string()
        };
        let pc = workspace_panes(ws).len();
        let pc_str = format!("{pc}");
        // budget: bar(1)+num(2)+dot(2)+name+pad+count+trailing(2)
        let name_budget = w.saturating_sub(1 + 2 + 2 + pc_str.len() + 3);
        let name = truncate(&ws.label, name_budget);
        let used = 1 + 2 + 2 + name.chars().count() + pc_str.len();
        let pad = w.saturating_sub(used + 2);
        let name_style = if active {
            Style::default().fg(p.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.subtext0)
        };
        lines.push(Line::from(vec![
            Span::styled(bar, bg.fg(bar_c)),
            Span::styled(num, bg.fg(if active { p.accent } else { p.overlay0 })),
            Span::styled("● ", bg.fg(ws.worst.color(p))),
            Span::styled(name, name_style.patch(bg)),
            Span::styled(" ".repeat(pad), bg),
            Span::styled(pc_str, bg.fg(p.overlay0)),
            Span::styled("  ", bg),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), list);
}

fn render_focus(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let ws = &deck.workspaces[st.active];
    if area.height < 2 {
        return;
    }
    let focused = st.focus == Column::Panes;
    let gutter = 2u16;
    let w = area.width.saturating_sub(gutter) as usize;

    // header: workspace name + rollup
    let head = Layout::horizontal([Constraint::Min(0), Constraint::Length(22)]).split(Rect::new(
        area.x + gutter,
        area.y,
        area.width.saturating_sub(gutter),
        1,
    ));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            ws.label.to_uppercase(),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ))),
        head[0],
    );
    let c = &ws.counts;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            rollup(p.red, "◉", c.blocked, p),
            Span::raw(" "),
            rollup(p.yellow, "◍", c.working, p),
            Span::raw(" "),
            rollup(p.teal, "●", c.done, p),
            Span::raw(" "),
            rollup(p.green, "✓", c.idle, p),
            Span::raw("  "),
        ]))
        .alignment(Alignment::Right),
        head[1],
    );

    // body: tab groups + panes, selected pane full-width highlighted
    let sel = st.sel.get(st.active).copied().unwrap_or(0);
    let mut lines: Vec<Line> = Vec::new();
    let mut sel_line = 0usize;
    let mut pane_idx = 0usize;
    for (ti, tab) in ws.tabs.iter().enumerate() {
        if ti > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(vec![
            Span::styled("  ▸ ", Style::default().fg(p.overlay0)),
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
            lines.push(pane_line(pane, selected, focused, w, p));
            pane_idx += 1;
        }
    }
    let body = Rect::new(
        area.x + gutter,
        area.y + 2,
        area.width.saturating_sub(gutter),
        area.height.saturating_sub(2),
    );
    let offset = scroll_offset(sel_line, body.height as usize, lines.len());
    frame.render_widget(Paragraph::new(lines).scroll((offset as u16, 0)), body);
}

/// A pane row; when selected it gets a full-width soft-background bar.
fn pane_line(pane: &Pane, selected: bool, focused: bool, width: usize, p: &Palette) -> Line<'static> {
    let bg = if selected {
        Style::default().bg(p.surface0)
    } else {
        Style::default()
    };
    let (bar, bar_c) = match (selected, focused) {
        (true, true) => ("▌ ", p.accent),
        (true, false) => ("▎ ", p.overlay0),
        (false, _) => ("  ", p.overlay0),
    };
    let label_style = if selected {
        Style::default().fg(p.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.subtext0)
    };
    let glyph = format!("{} ", pane.status.glyph());
    let head = bar.chars().count() + glyph.chars().count() + pane.label.chars().count();
    let pad = width.saturating_sub(head);
    Line::from(vec![
        Span::styled(bar, bg.fg(bar_c)),
        Span::styled(glyph, bg.fg(pane.status.color(p))),
        Span::styled(pane.label.clone(), label_style.patch(bg)),
        Span::styled(" ".repeat(pad), bg),
    ])
}

fn render_detail(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let pane = match st.mode {
        Mode::Browse => {
            let ws = &deck.workspaces[st.active];
            workspace_panes(ws)
                .get(st.sel.get(st.active).copied().unwrap_or(0))
                .map(|&(ti, pi)| &ws.tabs[ti].panes[pi])
        }
        Mode::Search => search_results(deck, &st.query)
            .get(st.result_sel)
            .map(|loc| &deck.workspaces[loc.wi].tabs[loc.ti].panes[loc.pi]),
    };
    let Some(pane) = pane else { return };

    let mut parts: Vec<String> = Vec::new();
    if let Some(cwd) = &pane.cwd {
        parts.push(shorten_path(cwd));
    }
    parts.push(pane.label.clone());
    if let Some(agent) = &pane.agent {
        parts.push(agent.clone());
    }
    parts.push(pane.status.label().to_string());
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {}", parts.join(" · ")),
            Style::default().fg(p.overlay0),
        ))),
        area,
    );
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
    let w = area.width.saturating_sub(2) as usize;
    let lines: Vec<Line> = hits
        .iter()
        .enumerate()
        .map(|(i, loc)| {
            let ws = &deck.workspaces[loc.wi];
            let tab = &ws.tabs[loc.ti];
            let pane = &tab.panes[loc.pi];
            let selected = i == st.result_sel;
            let bg = if selected {
                Style::default().bg(p.surface0)
            } else {
                Style::default()
            };
            let bar = if selected { "▌ " } else { "  " };
            let path = format!("{} ▸ {}", ws.label, tab.label);
            let label = truncate(&pane.label, 26);
            let head = bar.chars().count() + 2 + label.chars().count() + 2 + path.chars().count();
            let pad = w.saturating_sub(head);
            let lstyle = if selected {
                Style::default().fg(p.text).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(p.subtext0)
            };
            Line::from(vec![
                Span::styled(bar, bg.fg(p.accent)),
                Span::styled(format!("{} ", pane.status.glyph()), bg.fg(pane.status.color(p))),
                Span::styled(label, lstyle.patch(bg)),
                Span::styled(" ".repeat(pad), bg),
                Span::styled(path, bg.fg(p.overlay0)),
                Span::styled("  ", bg),
            ])
        })
        .collect();
    let offset = scroll_offset(st.result_sel, area.height as usize, lines.len());
    frame.render_widget(Paragraph::new(lines).scroll((offset as u16, 0)), area);
}

fn render_footer(frame: &mut Frame, area: Rect, st: &NavState, p: &Palette) {
    let line = match st.mode {
        Mode::Browse => Line::from(vec![
            key("← →", p), dim(" column   ", p),
            key("↑ ↓", p), dim(" move   ", p),
            key("1–9", p), dim(" jump   ", p),
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

fn shorten_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    path.to_string()
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
    use crate::state::{Column, NavState};
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
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked","label":"pane 1","cwd":"/tmp/esd","agent":"claude"},
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
    fn rail_and_focus_and_detail_render() {
        let s = draw(|st| st.active = 0, 90, 18);
        assert!(s.contains("WORKSPACES"), "rail header:\n{s}");
        assert!(s.contains("esd"), "shows esd:\n{s}");
        assert!(s.contains("load-generator"), "rail lists other ws:\n{s}");
        assert!(s.contains("pane 1"), "focus shows panes:\n{s}");
        assert!(s.contains("claude"), "detail strip shows agent:\n{s}");
    }

    #[test]
    fn search_mode_shows_filtered_results() {
        let s = draw(
            |st| {
                st.mode = crate::state::Mode::Search;
                st.query = "loadtest".into();
            },
            90,
            18,
        );
        assert!(s.contains("loadtest agent"), "result label:\n{s}");
        assert!(s.contains("load-generator ▸ lg-runner"), "result path:\n{s}");
    }

    #[test]
    fn focus_column_marks_pane_cursor() {
        let s = draw(
            |st| {
                st.active = 0;
                st.focus = Column::Panes;
            },
            90,
            18,
        );
        assert!(s.contains('▌'), "focused pane cursor bar:\n{s}");
    }

    #[test]
    fn does_not_panic_on_tiny_terminal() {
        let _ = draw(|_| {}, 24, 8);
        let _ = draw(|st| st.active = 1, 30, 10);
    }

    #[test]
    fn fills_opaque_theme_background() {
        // an empty body cell must carry the theme's panel bg (readability, not
        // a see-through wallpaper)
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let st = NavState::new(&deck);
        let pal = Palette::catppuccin();
        let mut term = Terminal::new(TestBackend::new(80, 18)).unwrap();
        term.draw(|f| render(f, &deck, &st, &pal)).unwrap();
        let buf = term.backend().buffer();
        assert_eq!(buf.cell((70, 10)).unwrap().bg, pal.panel_bg);
    }
}
