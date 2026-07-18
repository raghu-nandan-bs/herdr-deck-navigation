use crate::model::{Deck, Workspace};
use crate::state::{workspace_rows, NavState, Row};
use crate::theme::Palette;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

const CARD_W: u16 = 46;
const MAX_BACK: usize = 3; // how many stacked card-tops peek behind the front
const BACK_DX: u16 = 4; // horizontal offset per stacked card (fans the pile)

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

    let footer = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    let body = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1));

    let n = deck.workspaces.len();
    let front = &deck.workspaces[st.active];
    let cw = CARD_W.min(body.width);
    let ch = card_height(front, body.height);
    let cx = body.x + (body.width - cw) / 2;

    // Leave room above the front card for the stacked tops, but never push the
    // card off the bottom.
    let back = MAX_BACK.min(n.saturating_sub(1));
    let centered = body.y + body.height.saturating_sub(ch) / 2;
    let max_cy = body.y + body.height.saturating_sub(ch);
    let cy = centered.max(body.y + back as u16).min(max_cy);

    // The pile: other workspaces fan up-and-left behind the front card as
    // rounded card-tops, nearest just above it. Fixed position and direction
    // no matter where you navigate — the fan rotates, nothing flips sides.
    for d in (1..=back).rev() {
        let ws = &deck.workspaces[(st.active + d) % n];
        let x = cx.saturating_sub(d as u16 * BACK_DX);
        if let Some(y) = cy.checked_sub(d as u16) {
            if y >= body.y && x >= body.x {
                render_stack_top(frame, x, y, cw, ws, p);
            }
        }
    }

    render_card(frame, Rect::new(cx, cy, cw, ch), front, st, p);

    let hint = Line::from(vec![
        key("↵", p), dim(" switch   ", p),
        key("← →", p), dim(" workspace   ", p),
        key("↑ ↓", p), dim(" pane   ", p),
        key("b/w/i/d", p), dim(" filter   ", p),
        key("esc", p), dim(" close", p),
    ]);
    frame.render_widget(Paragraph::new(hint).alignment(Alignment::Center), footer);
}

/// One row: the rounded top edge of a card behind the front one, with a status
/// dot + workspace name — so the pile reads as real stacked cards.
fn render_stack_top(frame: &mut Frame, x: u16, y: u16, w: u16, ws: &Workspace, p: &Palette) {
    if w < 6 {
        return;
    }
    let name = truncate(&ws.label, (w as usize).saturating_sub(8));
    let used = 2 + 2 + name.chars().count() + 1; // "╭ " + "● " + name + " "
    let fill = (w as usize).saturating_sub(used + 1); // + "╮"
    let line = Line::from(vec![
        Span::styled("╭ ", Style::default().fg(p.overlay0)),
        Span::styled(
            format!("{} ", ws.worst.glyph()),
            Style::default().fg(ws.worst.color(p)),
        ),
        Span::styled(name, Style::default().fg(p.subtext0)),
        Span::styled(
            format!(" {}╮", "─".repeat(fill)),
            Style::default().fg(p.overlay0),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), Rect::new(x, y, w, 1));
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
    // wallpaper shows through it — a frosted-glass feel. Only the border, text,
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
        Style::default()
            .bg(p.accent)
            .fg(p.panel_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.text)
    };
    match row {
        Row::Workspace => Line::from(Span::styled("  workspace", base)),
        Row::Tab(ti) => Line::from(Span::styled(format!(" ▸ {}", ws.tabs[*ti].label), base)),
        Row::Pane(ti, pi) => {
            let pane = &ws.tabs[*ti].panes[*pi];
            let dimmed = st.filter.map_or(false, |f| f != pane.status);
            let glyph_style = if selected {
                base
            } else {
                Style::default().fg(pane.status.color(p))
            };
            let label_style = if selected {
                base
            } else if dimmed {
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
    fn stack_tops_show_other_workspaces() {
        let s = buffer_string(1, 80, 20);
        // front card is the active workspace; the pile behind shows the others
        assert!(s.contains("web"), "front card is web:\n{s}");
        assert!(s.contains("infra"), "stack top should show infra:\n{s}");
        assert!(s.contains("api"), "stack top should show api:\n{s}");
    }

    #[test]
    fn does_not_panic_on_tiny_terminal() {
        let _ = buffer_string(0, 12, 6);
        let _ = buffer_string(2, 20, 10);
    }
}
