use crate::model::{Deck, Workspace};
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

/// Height a card needs to show all its rows: 2 borders + rollup line + one row
/// per selectable row, capped to the available height (min 3 for a usable card).
fn card_height(ws: &Workspace, max: u16) -> u16 {
    (3 + workspace_rows(ws).len() as u16).min(max).max(3)
}

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

    if body.width < CARD_W + GAP {
        // Too narrow to fit a full card plus gap: render only the active
        // workspace's card, clamped to the available width and vertically
        // centered, so the body isn't left blank.
        if let Some(ws) = deck.workspaces.get(st.active) {
            let h = card_height(ws, body.height);
            let y = body.y + body.height.saturating_sub(h) / 2;
            let rect = Rect::new(body.x, y, body.width, h);
            render_card(frame, rect, ws, true, st);
        }
    } else {
        // How many cards fit, and where the horizontal scroll window starts
        // (keeps the active card in view when there are more cards than fit).
        let per_screen = ((body.width + GAP) / (CARD_W + GAP)).max(1) as usize;
        let start = st.active.saturating_sub(per_screen.saturating_sub(1));
        let visible = per_screen.min(deck.workspaces.len() - start);

        // Center the visible cards as a group when they don't fill the width.
        let total_w = visible as u16 * CARD_W + visible.saturating_sub(1) as u16 * GAP;
        let mut x = body.x + body.width.saturating_sub(total_w) / 2;

        for (wi, ws) in deck
            .workspaces
            .iter()
            .enumerate()
            .skip(start)
            .take(visible)
        {
            // Each card hugs its own content height and is centered vertically,
            // so cards with different row counts stay balanced on a common midline.
            let h = card_height(ws, body.height);
            let y = body.y + body.height.saturating_sub(h) / 2;
            let rect = Rect::new(x, y, CARD_W, h);
            render_card(frame, rect, ws, wi == st.active, st);
            x += CARD_W + GAP;
        }
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

    // left stripe: colored by the worst agent-status in this workspace
    let content = if inner.width < 2 {
        inner
    } else {
        let stripe = Rect::new(inner.x, inner.y, 1, inner.height);
        let bar: Vec<Line> = (0..inner.height)
            .map(|_| Line::from(Span::styled("▌", Style::default().fg(ws.worst.color()))))
            .collect();
        frame.render_widget(Paragraph::new(bar), stripe);
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(1), inner.height)
    };

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
        Rect::new(content.x, content.y, content.width, 1),
    );

    // rows
    let rows = workspace_rows(ws);
    let list_area = Rect::new(
        content.x,
        inner.y + 1,
        content.width,
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

    #[test]
    fn renders_worst_status_stripe() {
        // MINI's only workspace has a single blocked pane, so worst == Blocked
        // and the left stripe should be painted.
        let s = buffer_string(60, 20);
        assert!(s.contains('▌'), "should show worst-status stripe:\n{s}");
    }

    #[test]
    fn narrow_terminal_still_renders_active_card() {
        // body width (19) is less than CARD_W + GAP (31): the multi-card loop
        // would break immediately and leave a blank body, so the fallback
        // must render the active workspace's card clamped to the width.
        let s = buffer_string(20, 15);
        assert!(s.contains("api"), "should show active workspace label:\n{s}");
    }

    #[test]
    fn centers_the_single_card_horizontally_and_vertically() {
        // One small card in a large terminal must not hug the top-left corner:
        // it should be padded on both axes.
        let deck = build_deck(MINI).unwrap();
        let st = NavState::new(&deck);
        let (w, h) = (80u16, 30u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, &deck, &st)).unwrap();
        let buf = term.backend().buffer().clone();

        // locate the "api" title
        let mut pos = None;
        for y in 0..h {
            for x in 0..w.saturating_sub(3) {
                let s: String = (0..3)
                    .map(|i| buf.cell((x + i, y)).unwrap().symbol().to_string())
                    .collect();
                if s == "api" {
                    pos = Some((x, y));
                }
            }
        }
        let (cx, cy) = pos.expect("card title should render");
        assert!(cx > 15, "expected left padding (horizontal centering), got col {cx}");
        assert!(cy > 3, "expected top padding (vertical centering), got row {cy}");
    }
}
