//! CODEX//XTREME - Neo Tokyo Y2K Edition
//!
//! A cyberpunk-themed TUI for building patched Codex binaries.

use codex_xtreme::app::App;
use codex_xtreme::core::check_prerequisites;
use codex_xtreme::tui::{spawn_event_reader, TermEvent, Tui};
use ratatui::widgets::Widget;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse args
    let dev_mode = std::env::args().any(|a| a == "--dev" || a == "-d");

    if let Err(err) = check_prerequisites() {
        eprintln!("{err}");
        std::process::exit(1);
    }

    // Initialize TUI
    let mut tui = Tui::new()?;

    // Create app state
    let mut app = App::new(dev_mode);

    // Spawn event reader
    let mut events = spawn_event_reader();

    // Main loop
    loop {
        // Render
        tui.terminal().draw(|frame| {
            let area = frame.area();
            (&app.screen).render(area, frame.buffer_mut());
        })?;

        // Handle events
        if let Some(event) = events.recv().await {
            match event {
                TermEvent::Key(key) => {
                    app.handle_key(key);
                }
                TermEvent::Tick => {
                    app.tick();
                }
                TermEvent::Resize(_, _) => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    tui.restore()?;

    Ok(())
}
