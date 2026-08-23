use crate::model::{Deck, Pane};
use crate::state::{search_results, workspace_panes, Column, Mode, NavState};
use crate::theme::{Palette, Status};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

/// Rail width: wide enough for the longest workspace name (plus number, dot, and
/// pane count), capped so the focus pane keeps room.
fn natural_rail_width(deck: &Deck) -> u16 {
    let name_w = deck
        .workspaces
        .iter()
        .map(|w| w.label.chars().count())
        .max()
        .unwrap_or(8) as u16;
    // chrome ≈ bar + " N " + "◉ " + name + "  NN"
    name_w + 11
}

/// The rail gets the width its longest name needs, but never more than half the
/// panel — `card_rect` sizes the panel so that half is enough.
fn rail_width(deck: &Deck, area_w: u16) -> u16 {
    natural_rail_width(deck).clamp(22, (area_w / 2).clamp(22, 54))
}

/// Longest rendered line in the focus column, used to size the card.
fn focus_content_width(deck: &Deck) -> usize {
    deck.workspaces
        .iter()
        .flat_map(|w| w.tabs.iter())
        .flat_map(|t| {
            std::iter::once(t.label.chars().count() + 4)
                .chain(t.panes.iter().map(|p| p.label.chars().count() + 9))
        })
        .max()
        .unwrap_or(24)
}

/// The floating panel: sized to its content and centred, so a big terminal shows a
/// panel rather than a mostly-empty full-screen app. Falls back to the whole area
/// when the terminal is too small to float in.
pub fn card_rect(area: Rect, deck: &Deck) -> Rect {
    let rail_rows = 2 + deck.workspaces.len();
    let focus_rows = 2 + deck
        .workspaces
        .iter()
        .map(|w| {
            w.tabs.iter().map(|t| 1 + t.panes.len()).sum::<usize>() + w.tabs.len().saturating_sub(1)
        })
        .max()
        .unwrap_or(0);
    let body = rail_rows.max(focus_rows);

    let want_h = (body + 5 + 2) as u16; // chrome rows + borders
    let want_w = natural_rail_width(deck) + focus_content_width(deck) as u16 + 7;
    let w = want_w.clamp(68, 120).min(area.width);
    let h = want_h.min(area.height);
    // Float only with a real margin around it; otherwise the "panel" is just the
    // screen with a border drawn on it, and the margin is wasted rows.
    if area.width < w + 8 || area.height < h + 4 {
        return area;
    }
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

/// Brighten the top border to read as a lit glass edge.
fn glass_edge(frame: &mut Frame, card: Rect, p: &Palette) {
    if card.width < 3 {
        return;
    }
    let buf = frame.buffer_mut();
    for x in (card.x + 1)..(card.x + card.width - 1) {
        if let Some(cell) = buf.cell_mut((x, card.y)) {
            cell.set_fg(p.overlay0);
        }
    }
}

pub fn render(frame: &mut Frame, deck: &Deck, st: &NavState, p: &Palette) {
    let area = frame.area();
    if deck.workspaces.is_empty() || area.width < 24 || area.height < 8 {
        frame.render_widget(Block::default().style(Style::default().bg(p.panel_bg)), area);
        frame.render_widget(
            Paragraph::new("no workspaces").style(Style::default().fg(p.overlay0).bg(p.panel_bg)),
            area,
        );
        return;
    }

    // The panel is opaque (readable over any wallpaper); everything outside it is
    // left untouched, which is what makes it read as floating rather than full-bleed.
    let card = card_rect(area, deck);
    let shell = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.surface1))
        .style(Style::default().bg(p.panel_bg));
    let band = shell.inner(card);
    frame.render_widget(shell, card);
    glass_edge(frame, card, p);

    let v = Layout::vertical([
        Constraint::Length(1), // search bar
        Constraint::Length(1), // divider
        Constraint::Min(0),    // body
        Constraint::Length(1), // divider
        Constraint::Length(1), // detail strip
        Constraint::Length(1), // footer
    ])
    .split(band);

    // Frosted chrome: header and footer bands sit a shade above the panel.
    for row in [v[0], v[5]] {
        frame.render_widget(Block::default().style(Style::default().bg(p.surface0)), row);
    }

    // Pad the content in by a column so nothing hugs the border.
    let pad = |r: Rect| Rect::new(r.x + 1, r.y, r.width.saturating_sub(2), r.height);

    render_topbar(frame, pad(v[0]), deck, st, p);
    hrule(frame, card, v[1].y, p);

    match st.mode {
        Mode::Browse => {
            let body = pad(v[2]);
            let rw = rail_width(deck, body.width);
            let h = Layout::horizontal([Constraint::Length(rw), Constraint::Min(0)]).split(body);
            render_rail(frame, h[0], deck, st, p);
            render_focus(frame, h[1], deck, st, p);
        }
        Mode::Search => render_results(frame, pad(v[2]), deck, st, p),
    }

    hrule(frame, card, v[3].y, p);
    render_detail(frame, pad(v[4]), deck, st, p);
    render_footer(frame, pad(v[5]), st, p);
}

/// A divider drawn edge to edge, joining the rounded border with ├ ┤ rather than
/// butting a bare rule against the wall.
fn hrule(frame: &mut Frame, card: Rect, y: u16, p: &Palette) {
    let inner = card.width.saturating_sub(2) as usize;
    frame.render_widget(
        Paragraph::new(format!("├{}┤", "─".repeat(inner)))
            .style(Style::default().fg(p.surface1).bg(p.panel_bg)),
        Rect::new(card.x, y, card.width, 1),
    );
}

/// What the top-right corner says. When agents want you, it says so — "3 agents
/// finished while you were away" is the most useful sentence this tool can show.
/// When nothing does, it falls back to the plain pane count.
fn attention_summary(deck: &Deck) -> String {
    let (mut blocked, mut done, mut total) = (0usize, 0usize, 0usize);
    for pane in deck.workspaces.iter().flat_map(|w| w.tabs.iter()).flat_map(|t| t.panes.iter()) {
        total += 1;
        match pane.status {
            Status::Blocked => blocked += 1,
            Status::Done => done += 1,
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if blocked > 0 {
        parts.push(format!("{blocked} blocked"));
    }
    if done > 0 {
        parts.push(format!("{done} done"));
    }
    if parts.is_empty() {
        format!("{total} panes")
    } else {
        parts.join(" · ")
    }
}

fn render_topbar(frame: &mut Frame, area: Rect, deck: &Deck, st: &NavState, p: &Palette) {
    let summary = attention_summary(deck);
    let urgent = deck
        .workspaces
        .iter()
        .flat_map(|w| w.tabs.iter())
        .flat_map(|t| t.panes.iter())
        .any(|p| matches!(p.status, Status::Blocked));
    let right_w = (summary.chars().count() + 2) as u16;
    let cols = Layout::horizontal([Constraint::Min(0), Constraint::Length(right_w)]).split(area);

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
        // Truncated to its own column so it can never run into the summary.
        Mode::Browse => left.push(Span::styled(
            truncate(
                "tab for whoever needs you · / to search",
                (cols[0].width as usize).saturating_sub(2),
            ),
            Style::default().fg(p.overlay0),
        )),
    }
    frame.render_widget(
        Paragraph::new(Line::from(left)).style(Style::default().bg(p.surface0)),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(format!("{summary}  "))
            .alignment(Alignment::Right)
            .style(Style::default().fg(if urgent { p.red } else { p.overlay0 }).bg(p.surface0)),
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
            Span::styled(format!("{} ", ws.worst.glyph()), bg.fg(ws.worst.color(p))),
            Span::styled(name, name_style.patch(bg)),
            Span::styled(" ".repeat(pad), bg),
            Span::styled(pc_str, bg.fg(p.overlay0)),
            Span::styled("  ", bg),
        ]));
    }
    let offset = scroll_offset(st.active, list.height as usize, lines.len());
    frame.render_widget(Paragraph::new(lines).scroll((offset as u16, 0)), list);
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
    let c = &ws.counts;
    let pips: Vec<(ratatui::style::Color, &str, usize)> = [
        (p.red, "◉", c.blocked),
        (p.yellow, "◍", c.working),
        (p.teal, "●", c.done),
        (p.green, "✓", c.idle),
    ]
    .into_iter()
    .filter(|&(_, _, n)| n > 0)
    .collect();
    let pips_w = pips.iter().map(|(_, _, n)| 2 + n.to_string().len()).sum::<usize>() as u16 + 2;
    let head = Layout::horizontal([Constraint::Min(0), Constraint::Length(pips_w)]).split(Rect::new(
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
    let mut spans: Vec<Span> = Vec::new();
    for (color, glyph, n) in pips {
        spans.push(Span::styled(
            format!("{glyph}{n}"),
            Style::default().fg(color),
        ));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
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
    // The pane you're sitting in right now, so `r` and Enter have an anchor.
    let tag = if pane.is_current { "here  " } else { "" };
    let head = bar.chars().count() + glyph.chars().count() + pane.label.chars().count();
    let pad = width.saturating_sub(head + tag.chars().count());
    Line::from(vec![
        Span::styled(bar, bg.fg(bar_c)),
        Span::styled(glyph, bg.fg(pane.status.color(p))),
        Span::styled(pane.label.clone(), label_style.patch(bg)),
        Span::styled(" ".repeat(pad), bg),
        Span::styled(tag, bg.fg(p.overlay0)),
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
    // Panes are often labelled after their agent; saying "claude · claude" is noise.
    if let Some(agent) = pane.agent.as_deref().filter(|a| *a != pane.label) {
        parts.push(agent.to_string());
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

/// Browse-mode footer hints, most important first, trimmed from the end until they
/// fit `width`. `tab` is the product, so it goes first and is the last to go.
fn browse_hints(width: usize) -> Vec<(&'static str, &'static str)> {
    let all: [(&str, &str); 7] = [
        ("tab", " needs you   "),
        ("r", " resume   "),
        ("← →", " column   "),
        ("↑ ↓", " move   "),
        ("/", " search   "),
        ("↵", " switch   "),
        ("esc", " close"),
    ];
    let mut hints = all.to_vec();
    while hints.len() > 1
        && hints
            .iter()
            .map(|(k, d)| k.chars().count() + d.chars().count())
            .sum::<usize>()
            > width
    {
        hints.pop();
    }
    hints
}

fn render_footer(frame: &mut Frame, area: Rect, st: &NavState, p: &Palette) {
    let line = match st.mode {
        Mode::Browse => Line::from(
            browse_hints(area.width as usize)
                .into_iter()
                .flat_map(|(k, d)| [key(k, p), dim(d, p)])
                .collect::<Vec<_>>(),
        ),
        Mode::Search => Line::from(vec![
            dim("type to filter   ", p),
            key("↑ ↓", p), dim(" select   ", p),
            key("↵", p), dim(" switch   ", p),
            key("esc", p), dim(" back", p),
        ]),
    };
    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(Style::default().bg(p.surface0)),
        area,
    );
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


    /// Render an arbitrary snapshot, with a hook to adjust the cursor first.
    fn draw_json(json: &str, setup: impl FnOnce(&mut NavState), w: u16, h: u16) -> String {
        let deck = build_deck(json, &Context::default()).unwrap();
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

    /// `n` workspaces, one idle pane each — more than fit in a short rail.
    fn many_workspaces(n: usize) -> String {
        let ws: Vec<String> = (1..=n)
            .map(|i| format!(r#"{{"workspace_id":"w{i}","label":"ws-{i}","number":{i}}}"#))
            .collect();
        let tabs: Vec<String> = (1..=n)
            .map(|i| format!(r#"{{"tab_id":"w{i}:t1","workspace_id":"w{i}","label":"t","number":1}}"#))
            .collect();
        let panes: Vec<String> = (1..=n)
            .map(|i| format!(
                r#"{{"pane_id":"w{i}:p1","tab_id":"w{i}:t1","workspace_id":"w{i}","agent_status":"idle","label":"p{i}"}}"#
            ))
            .collect();
        format!(
            r#"{{"result":{{"snapshot":{{"workspaces":[{}],"tabs":[{}],"panes":[{}]}}}}}}"#,
            ws.join(","), tabs.join(","), panes.join(",")
        )
    }

    #[test]
    fn rail_scrolls_to_keep_the_active_workspace_visible() {
        let json = many_workspaces(20);
        // 14 rows total leaves the rail ~8 lines: ws-20 only shows if the rail scrolls.
        let s = draw_json(&json, |st| st.active = 19, 90, 14);
        assert!(s.contains("ws-20"), "active workspace must be visible:\n{s}");
        assert!(!s.contains("ws-1 "), "top of a scrolled rail is off-screen:\n{s}");
    }

    #[test]
    fn attention_summary_counts_blocked_and_done_across_the_deck() {
        let calm = build_deck(&many_workspaces(2), &Context::default()).unwrap();
        assert_eq!(attention_summary(&calm), "2 panes");

        let busy = build_deck(
            r#"{"result":{"snapshot":{
              "workspaces":[{"workspace_id":"w1","label":"a","number":1}],
              "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
              "panes":[
                {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked"},
                {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"done"},
                {"pane_id":"w1:p3","tab_id":"w1:t1","workspace_id":"w1","agent_status":"done"},
                {"pane_id":"w1:p4","tab_id":"w1:t1","workspace_id":"w1","agent_status":"idle"}
              ]
            }}}"#,
            &Context::default(),
        )
        .unwrap();
        assert_eq!(attention_summary(&busy), "1 blocked · 2 done");
    }

    #[test]
    fn topbar_shows_the_attention_summary() {
        let s = draw(|_| {}, 90, 18); // MINI has one blocked pane
        assert!(s.contains("1 blocked"), "topbar summary:\n{s}");
    }

    #[test]
    fn footer_offers_tab_and_resume() {
        let s = draw(|_| {}, 100, 18);
        assert!(s.contains("tab"), "tab hint:\n{s}");
        assert!(s.contains("resume"), "resume hint:\n{s}");
    }

    fn footer_width(hints: &[(&str, &str)]) -> usize {
        hints.iter().map(|(k, d)| k.chars().count() + d.chars().count()).sum()
    }

    #[test]
    fn footer_drops_the_least_important_hints_to_fit_a_narrow_terminal() {
        let wide = browse_hints(120);
        assert!(footer_width(&wide) <= 120);
        assert_eq!(wide.len(), 7, "everything fits at 120 cols");

        for w in [40usize, 60, 78, 90] {
            let hints = browse_hints(w);
            assert!(footer_width(&hints) <= w, "clips at {w}: {hints:?}");
            assert_eq!(hints[0].0, "tab", "tab survives every width");
        }
    }

    #[test]
    fn footer_keeps_the_core_hints_at_the_documented_minimum_width() {
        // 24 cols is the narrowest terminal render() will draw at all.
        let hints = browse_hints(24);
        assert!(footer_width(&hints) <= 24);
        assert!(!hints.is_empty());
    }

    #[test]
    fn topbar_hint_never_collides_with_the_summary() {
        // A long summary ("1 blocked · 2 done") plus the hint overflows 56 cols.
        let json = r#"{"result":{"snapshot":{
          "workspaces":[{"workspace_id":"w1","label":"a","number":1}],
          "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
          "panes":[
            {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked"},
            {"pane_id":"w1:p2","tab_id":"w1:t1","workspace_id":"w1","agent_status":"done"},
            {"pane_id":"w1:p3","tab_id":"w1:t1","workspace_id":"w1","agent_status":"done"}]
        }}}"#;
        let s = draw_json(json, |_| {}, 56, 14);
        let top = s.lines().find(|l| l.contains("blocked")).expect("topbar row");
        assert!(top.contains("1 blocked · 2 done"), "summary intact:\n{top}");
        assert!(top.contains(" 1 blocked"), "summary needs clear space:\n{top}");
    }

    #[test]
    fn card_is_centred_and_sized_to_content_not_the_whole_screen() {
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let area = Rect::new(0, 0, 200, 50);
        let card = card_rect(area, &deck);
        assert!(card.width < area.width - 8, "leaves side margin: {card:?}");
        assert!(card.height < area.height - 8, "leaves top/bottom margin: {card:?}");
        // centred within a cell of slack
        let lgap = card.x;
        let rgap = area.width - (card.x + card.width);
        assert!(lgap.abs_diff(rgap) <= 1, "horizontally centred: {lgap} vs {rgap}");
        let tgap = card.y;
        let bgap = area.height - (card.y + card.height);
        assert!(tgap.abs_diff(bgap) <= 1, "vertically centred: {tgap} vs {bgap}");
    }

    #[test]
    fn card_grows_to_fill_a_small_terminal() {
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let area = Rect::new(0, 0, 60, 14);
        let card = card_rect(area, &deck);
        assert_eq!((card.x, card.y), (0, 0));
        assert_eq!((card.width, card.height), (60, 14));
    }

    #[test]
    fn panel_floats_opaque_over_an_untouched_background() {
        // Outside the card the terminal shows through (that's the float); inside,
        // the panel is fully opaque so it stays readable over any wallpaper.
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let st = NavState::new(&deck);
        let pal = Palette::catppuccin();
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| render(f, &deck, &st, &pal)).unwrap();
        let buf = term.backend().buffer();
        assert_eq!(buf.cell((0, 0)).unwrap().bg, ratatui::style::Color::Reset, "outside is transparent");
        let card = card_rect(Rect::new(0, 0, 120, 40), &deck);
        let mid = (card.x + card.width / 2, card.y + card.height / 2);
        assert_ne!(buf.cell(mid).unwrap().bg, ratatui::style::Color::Reset, "inside is painted");
    }

    #[test]
    fn panel_has_rounded_corners() {
        let s = draw(|_| {}, 120, 40);
        assert!(s.contains('╭') && s.contains('╮'), "rounded top:\n{s}");
        assert!(s.contains('╰') && s.contains('╯'), "rounded bottom:\n{s}");
    }

    #[test]
    fn rail_uses_the_status_glyph_not_a_flat_dot() {
        // MINI's esd workspace holds a blocked pane — the rail must say so with
        // the blocked glyph, not colour alone (colour is not readable for everyone).
        let s = draw(|_| {}, 120, 40);
        let rail_row = s
            .lines()
            .find(|l| l.contains("esd"))
            .expect("rail lists esd");
        assert!(rail_row.contains('◉'), "blocked glyph in rail:\n{rail_row}");
    }

    #[test]
    fn rollup_omits_zero_counts() {
        // esd: 1 blocked, nothing else — so only the blocked pip is shown.
        let s = draw(|_| {}, 120, 40);
        let header = s.lines().find(|l| l.contains("ESD")).expect("focus header");
        assert!(header.contains("◉1"), "shows the non-zero count:\n{header}");
        assert!(!header.contains("◍0"), "hides zero counts:\n{header}");
        assert!(!header.contains("●0"), "hides zero counts:\n{header}");
    }

    #[test]
    fn the_pane_you_are_in_is_marked() {
        // MINI focuses w1:p1 ("pane 1"); it should be visibly tagged as where you are.
        let s = draw(|st| st.active = 0, 120, 40);
        let row = s.lines().find(|l| l.contains("pane 1")).expect("pane row");
        assert!(row.contains('▸') || row.contains("here"), "current-pane marker:\n{row}");
    }

    #[test]
    fn dividers_join_the_panel_border_instead_of_breaking_it() {
        let s = draw(|_| {}, 120, 40);
        assert!(s.contains('├') && s.contains('┤'), "tee junctions:\n{s}");
        assert!(!s.contains("│──"), "no divider butting against a wall:\n{s}");
    }

    #[test]
    fn panel_height_tracks_content_without_reserving_dead_rows() {
        // 7 workspaces x 2 panes: the rail needs 9 rows and the widest focus
        // column 5, so the body is 9 — the 14 panes a search would list must not
        // inflate it into rows that sit empty while browsing.
        let (mut ws, mut tabs, mut panes) = (vec![], vec![], vec![]);
        for i in 1..=7 {
            ws.push(format!(r#"{{"workspace_id":"w{i}","label":"ws{i}","number":{i}}}"#));
            tabs.push(format!(r#"{{"tab_id":"w{i}:t1","workspace_id":"w{i}","label":"t","number":1}}"#));
            for j in 1..=2 {
                panes.push(format!(r#"{{"pane_id":"w{i}:p{j}","tab_id":"w{i}:t1","workspace_id":"w{i}","agent_status":"idle","label":"p"}}"#));
            }
        }
        let json = format!(
            r#"{{"result":{{"snapshot":{{"workspaces":[{}],"tabs":[{}],"panes":[{}]}}}}}}"#,
            ws.join(","), tabs.join(","), panes.join(",")
        );
        let deck = build_deck(&json, &Context::default()).unwrap();
        let card = card_rect(Rect::new(0, 0, 200, 60), &deck);
        assert_eq!(card.height, 9 + 5 + 2, "body + chrome + borders, nothing spare");
    }

    #[test]
    fn detail_strip_does_not_repeat_the_agent_as_the_label() {
        let json = r#"{"result":{"snapshot":{
          "focused_workspace_id":"w1","focused_pane_id":"w1:p1",
          "workspaces":[{"workspace_id":"w1","label":"a","number":1}],
          "tabs":[{"tab_id":"w1:t1","workspace_id":"w1","label":"t","number":1}],
          "panes":[{"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked",
                    "label":"claude","cwd":"/Users/me/code","agent":"claude"}]
        }}}"#;
        let s = draw_json(json, |_| {}, 120, 40);
        let row = s.lines().find(|l| l.contains("/Users/me/code")).expect("detail strip");
        assert!(!row.contains("claude · claude"), "agent said twice:\n{row}");
        assert!(row.contains("claude"), "agent still shown:\n{row}");
    }
}
