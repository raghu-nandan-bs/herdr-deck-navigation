use crate::model::{Deck, Workspace};
use crate::state::{workspace_rows, NavState, Row};
use crate::theme::Palette;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

const CARD_W: u16 = 44;
const NB_GAP: u16 = 2; // gap between fanned neighbour cards
const NB_SHRINK: u16 = 7; // each neighbour is this much narrower than the last
const NB_MIN: u16 = 16; // smallest neighbour width worth drawing
const MAX_SIDE: usize = 3; // how many neighbours to fan per side

/// Rows a card needs: 2 borders + rollup line + one row per selectable row.
fn card_height(ws: &Workspace, max: u16) -> u16 {
    (3 + workspace_rows(ws).len() as u16).min(max).max(4)
}

pub fn render(frame: &mut Frame, deck: &Deck, st: &NavState, p: &Palette) {
    let area = frame.area();
    if deck.workspaces.is_empty() || area.width < 14 || area.height < 6 {
        frame.render_widget(
            Paragraph::new("no workspaces").style(Style::default().fg(p.overlay0)),
            area,
        );
        return;
    }

    let top = Rect::new(area.x, area.y, area.width, 1);
    let footer = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 2);

    let cw = CARD_W.min(body.width);
    let front = &deck.workspaces[st.active];
    let ch = card_height(front, body.height);
    let cx = body.x + (body.width - cw) / 2;
    let cy = body.y + (body.height - ch) / 2;

    // Upcoming workspaces fan out to the LEFT (dimmer + smaller), already-seen
    // ones to the RIGHT. They sit beside the front card (no overlap), so the
    // glass/transparent fill stays clean.
    let mut left_edge = cx;
    for d in 1..=MAX_SIDE {
        let Some(ws) = deck.workspaces.get(st.active + d) else {
            break;
        };
        let w = cw.saturating_sub(d as u16 * NB_SHRINK).max(NB_MIN);
        let h = ch.saturating_sub(d as u16 * 2).max(5);
        let Some(x) = left_edge.checked_sub(NB_GAP + w) else {
            break;
        };
        if x < body.x {
            break;
        }
        let y = body.y + (body.height - h) / 2;
        render_neighbor(frame, Rect::new(x, y, w, h), ws, p);
        left_edge = x;
    }
    let mut right_edge = cx + cw;
    for d in 1..=MAX_SIDE {
        if st.active < d {
            break;
        }
        let Some(ws) = deck.workspaces.get(st.active - d) else {
            break;
        };
        let w = cw.saturating_sub(d as u16 * NB_SHRINK).max(NB_MIN);
        let h = ch.saturating_sub(d as u16 * 2).max(5);
        let x = right_edge + NB_GAP;
        if x + w > body.x + body.width {
            break;
        }
        let y = body.y + (body.height - h) / 2;
        render_neighbor(frame, Rect::new(x, y, w, h), ws, p);
        right_edge = x + w;
    }

    render_card(frame, Rect::new(cx, cy, cw, ch), front, st, p);

    // top counter / position
    let mut head = vec![];
    if st.active > 0 {
        head.push(Span::styled("‹  ", Style::default().fg(p.overlay0)));
    }
    head.push(Span::styled(
        front.label.clone(),
        Style::default().fg(p.text).add_modifier(Modifier::BOLD),
    ));
    head.push(Span::styled(
        format!("   {} of {}", st.active + 1, deck.workspaces.len()),
        Style::default().fg(p.overlay0),
    ));
    if st.active + 1 < deck.workspaces.len() {
        head.push(Span::styled("  ›", Style::default().fg(p.overlay0)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(head)).alignment(ratatui::layout::Alignment::Center),
        top,
    );

    let hint = Line::from(vec![
        key("↵", p), dim(" switch   ", p),
        key("← →", p), dim(" workspace   ", p),
        key("↑ ↓", p), dim(" pane   ", p),
        key("b/w/i/d", p), dim(" filter   ", p),
        key("esc", p), dim(" close", p),
    ]);
    frame.render_widget(
        Paragraph::new(hint).alignment(ratatui::layout::Alignment::Center),
        footer,
    );
}

/// A dimmed, compact neighbour card in the fan: rounded border, name, worst-status
/// stripe, rollup, and its tab list — enough to read as another workspace card.
fn render_neighbor(frame: &mut Frame, rect: Rect, ws: &Workspace, p: &Palette) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            ws.label.clone(),
            Style::default().fg(p.subtext0).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.overlay0))
        .title(title);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height == 0 || inner.width < 2 {
        return;
    }

    // worst-status stripe
    let bar: Vec<Line> = (0..inner.height)
        .map(|_| Line::from(Span::styled("▌", Style::default().fg(ws.worst.color(p)))))
        .collect();
    frame.render_widget(Paragraph::new(bar), Rect::new(inner.x, inner.y, 1, inner.height));
    let content = Rect::new(inner.x + 1, inner.y, inner.width - 1, inner.height);

    let rollup = Line::from(vec![
        rollup_span(p.red, "◉", ws.counts.blocked, p),
        Span::raw(" "),
        rollup_span(p.yellow, "◍", ws.counts.working, p),
        Span::raw(" "),
        rollup_span(p.teal, "●", ws.counts.done, p),
        Span::raw(" "),
        rollup_span(p.green, "✓", ws.counts.idle, p),
    ]);
    frame.render_widget(
        Paragraph::new(rollup),
        Rect::new(content.x, content.y, content.width, 1),
    );

    // dim tab list to give the card substance
    let tabs: Vec<Line> = ws
        .tabs
        .iter()
        .map(|t| {
            Line::from(Span::styled(
                format!(" ▸ {}", t.label),
                Style::default().fg(p.overlay0),
            ))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(tabs),
        Rect::new(
            content.x,
            content.y + 1,
            content.width,
            content.height.saturating_sub(1),
        ),
    );
}

fn render_card(frame: &mut Frame, rect: Rect, ws: &Workspace, st: &NavState, p: &Palette) {
    let title = Line::from(vec![
        Span::raw(" "),
        if ws.is_current {
            Span::styled("◆ ", Style::default().fg(p.accent))
        } else {
            Span::raw("")
        },
        Span::styled(
            ws.label.clone(),
            Style::default().fg(p.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);
    // No background fill: the card is transparent so the terminal's (blurred)
    // wallpaper shows through it — real frosted glass. Only the border, text,
    // stripe, and the selected-row highlight paint pixels.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(p.accent))
        .title(title);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if inner.height == 0 || inner.width < 2 {
        return;
    }

    // worst-status stripe on the inner-left edge
    let stripe = Rect::new(inner.x, inner.y, 1, inner.height);
    let bar: Vec<Line> = (0..inner.height)
        .map(|_| Line::from(Span::styled("▌", Style::default().fg(ws.worst.color(p)))))
        .collect();
    frame.render_widget(Paragraph::new(bar), stripe);
    let content = Rect::new(inner.x + 1, inner.y, inner.width - 1, inner.height);

    // rollup line
    let rollup = Line::from(vec![
        rollup_span(p.red, "◉", ws.counts.blocked, p),
        Span::raw(" "),
        rollup_span(p.yellow, "◍", ws.counts.working, p),
        Span::raw(" "),
        rollup_span(p.teal, "●", ws.counts.done, p),
        Span::raw(" "),
        rollup_span(p.green, "✓", ws.counts.idle, p),
    ]);
    frame.render_widget(
        Paragraph::new(rollup),
        Rect::new(content.x, content.y, content.width, 1),
    );

    // rows
    let rows = workspace_rows(ws);
    let sel = st.sel.get(st.active).copied().unwrap_or(0);
    let list = Rect::new(
        content.x,
        content.y + 1,
        content.width,
        content.height.saturating_sub(1),
    );
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(ri, row)| render_row(ws, row, ri == sel, st, p))
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

fn render_row(ws: &Workspace, row: &Row, selected: bool, st: &NavState, p: &Palette) -> Line<'static> {
    let base = if selected {
        Style::default().bg(p.accent).fg(p.panel_bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.text)
    };
    match row {
        Row::Workspace => Line::from(Span::styled("  workspace", base)),
        Row::Tab(ti) => Line::from(vec![
            Span::styled(format!(" ▸ {}", ws.tabs[*ti].label), base),
        ]),
        Row::Pane(ti, pi) => {
            let pane = &ws.tabs[*ti].panes[*pi];
            let dim = st.filter.map_or(false, |f| f != pane.status);
            let glyph_style = if selected {
                base
            } else {
                Style::default().fg(pane.status.color(p))
            };
            let label_style = if selected {
                base
            } else if dim {
                Style::default().fg(p.overlay0)
            } else {
                Style::default().fg(p.subtext0)
            };
            Line::from(vec![
                Span::styled("   ", base),
                Span::styled(pane.status.glyph().to_string(), glyph_style),
                Span::styled(format!(" {}", pane.label), label_style),
            ])
        }
    }
}

fn rollup_span(color: ratatui::style::Color, glyph: &str, n: usize, p: &Palette) -> Span<'static> {
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
      "focused_workspace_id":"w1","focused_tab_id":"w1:t1","focused_pane_id":"w1:p1",
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
        {"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1","agent_status":"blocked","label":"loadtest agent"},
        {"pane_id":"w2:p1","tab_id":"w2:t1","workspace_id":"w2","agent_status":"idle"},
        {"pane_id":"w3:p1","tab_id":"w3:t1","workspace_id":"w3","agent_status":"working"}
      ]
    }}}"#;

    fn buffer_string(active: usize, w: u16, h: u16) -> String {
        let deck = build_deck(MINI, &Context::default()).unwrap();
        let mut st = NavState::new(&deck);
        st.active = active;
        let pal = Palette::catppuccin();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, &deck, &st, &pal)).unwrap();
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
    fn renders_front_card_label_and_glyph() {
        let s = buffer_string(0, 80, 20);
        assert!(s.contains("api"), "front workspace label:\n{s}");
        assert!(s.contains("loadtest agent"), "real pane label:\n{s}");
        assert!(s.contains("◉"), "blocked glyph:\n{s}");
    }

    #[test]
    fn shows_position_counter() {
        let s = buffer_string(1, 80, 20);
        assert!(s.contains("2 of 3"), "position counter:\n{s}");
    }

    #[test]
    fn does_not_panic_on_tiny_terminal() {
        let _ = buffer_string(0, 12, 6);
        let _ = buffer_string(2, 20, 10);
    }
}
