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
    let ctx = model::Context::from_env();
    let palette = theme::Palette::resolve();
    let snapshot = client::snapshot(&path)?;
    let mut deck = model::build_deck(&snapshot, &ctx)?;
    if deck.workspaces.is_empty() {
        eprintln!("herdr-deck: no workspaces");
        return Ok(());
    }
    let mut st = NavState::new(&deck);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut term, &path, &ctx, &mut deck, &mut st, &palette);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;

    // Perform the focus action AFTER restoring the terminal so the popup closes cleanly.
    match result {
        Ok(Some(target)) => client::focus(&path, &target)?,
        Ok(None) => {}
        Err(e) => return Err(e),
    }
    Ok(())
}

fn run(
    term: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    path: &std::path::Path,
    ctx: &model::Context,
    deck: &mut model::Deck,
    st: &mut NavState,
    palette: &theme::Palette,
) -> Result<Option<state::FocusTarget>> {
    use std::time::Duration;
    loop {
        term.draw(|f| ui::render(f, deck, st, palette))?;
        // Poll so a left-open deck refreshes itself: renames, new panes, and
        // agent-status changes show up without reopening.
        if event::poll(Duration::from_millis(1000))? {
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
        } else if let Ok(snapshot) = client::snapshot(path) {
            // idle tick: re-fetch and rebuild, keeping the cursor in place
            if let Ok(fresh) = model::build_deck(&snapshot, ctx) {
                *deck = fresh;
                st.reconcile(deck);
            }
        }
    }
}
