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
    let snapshot = client::snapshot(&path)?;
    let deck = model::build_deck(&snapshot)?;
    if deck.workspaces.is_empty() {
        eprintln!("herdr-deck: no workspaces");
        return Ok(());
    }
    let mut st = NavState::new(&deck);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = run(&mut term, &deck, &mut st);

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
    deck: &model::Deck,
    st: &mut NavState,
) -> Result<Option<state::FocusTarget>> {
    loop {
        term.draw(|f| ui::render(f, deck, st))?;
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
    }
}
