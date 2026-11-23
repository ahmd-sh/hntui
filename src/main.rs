mod api;
mod app;
mod canvas;
mod clipboard;
mod color;
mod config;
mod format;
mod notes;
mod spinner;
mod store;
mod theme;
mod tree;
mod ui;
mod update;
mod wipe;
mod wrap;

use std::time::Duration;

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::execute;
use tokio::sync::mpsc;

use app::{Action, App, Msg};
use canvas::Canvas;
use config::ThemeSetting;
use theme::ThemeName;
use wipe::Wipe;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
usage: hntui [--theme dark|light] [--mouse | --no-mouse]
       hntui update           update to the latest release
       hntui config [edit]    show (or edit) the config file
       hntui changelog        what's new in this version
       hntui --version";

fn fail(msg: &str) -> ! {
    eprintln!("hntui: {msg}\n{USAGE}");
    std::process::exit(2);
}

// flags for this run; each overrides the matching config.toml setting
#[derive(Default)]
struct Flags {
    theme: Option<ThemeName>,
    mouse: Option<bool>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut flags = Flags::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-v" | "version" => {
                println!("hntui {VERSION}");
                return Ok(());
            }
            "--help" | "-h" | "help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "update" if i == 0 => std::process::exit(update::run().await),
            "config" if i == 0 => {
                let edit = match args.get(1).map(String::as_str) {
                    None => false,
                    Some("edit") => true,
                    Some(other) => fail(&format!("unknown config command \"{other}\"")),
                };
                if let Err(e) = config::command(edit) {
                    eprintln!("hntui: {e}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            "changelog" if i == 0 => {
                notes::print(VERSION);
                return Ok(());
            }
            "--mouse" => flags.mouse = Some(true),
            "--no-mouse" => flags.mouse = Some(false),
            "--theme" => {
                i += 1;
                flags.theme = Some(match args.get(i).map(String::as_str) {
                    Some("dark") => ThemeName::Dark,
                    Some("light") => ThemeName::Light,
                    _ => fail("--theme takes dark or light"),
                });
            }
            arg => fail(&format!("unknown option \"{arg}\"")),
        }
        i += 1;
    }

    // flags beat config.toml, which beats what the app remembered
    let (cfg, cfg_warning) = config::load();
    let mut state = store::load_state();
    let theme = flags.theme.unwrap_or(match cfg.theme {
        ThemeSetting::Fixed(t) => t,
        ThemeSetting::Last => state.theme.unwrap_or(ThemeName::Dark),
    });

    // Release notes, once per update. No record of having shown any means
    // a first run: either an upgrade from a release that didn't keep one
    // (it left saved posts or history behind) or a fresh install.
    let notes = match &state.notes_seen {
        Some(seen) => notes::since(Some(seen), VERSION),
        None if !store::load_saved().is_empty() || !store::load_history().is_empty() => notes::since(None, VERSION),
        None => vec![],
    };
    if state.notes_seen.as_deref() != Some(VERSION) {
        state.notes_seen = Some(VERSION.to_string());
        store::persist_state(&state);
    }
    let mouse = flags.mouse.unwrap_or(cfg.mouse);

    // raw mode + alternate screen + a panic hook that restores the terminal
    let mut terminal = ratatui::init();
    if mouse {
        let _ = execute!(std::io::stdout(), EnableMouseCapture);
        // ratatui's hook restores the screen but not mouse reporting; without
        // this a crash leaves the shell printing escape codes on every move
        let restore = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = execute!(std::io::stdout(), DisableMouseCapture);
            restore(info);
        }));
    }
    let (tx, mut rx) = mpsc::unbounded_channel();

    // crossterm's blocking reader on its own thread, forwarding into the loop
    let input = tx.clone();
    std::thread::spawn(move || loop {
        let msg = match event::read() {
            Ok(Event::Key(k)) => Msg::Key(k),
            Ok(Event::Mouse(m)) => Msg::Mouse(m),
            Ok(Event::Resize(..)) => Msg::Resize,
            Ok(_) => continue,
            Err(_) => break,
        };
        if input.send(msg).is_err() {
            break;
        }
    });

    let mut app = App::new(tx, api::Hn::new(), theme);
    if !notes.is_empty() {
        app.notes = Some(app::Notes { sections: notes, scroll: 0, max: 0 });
    }
    if let Some(w) = cfg_warning {
        app.notify(w, true, Duration::from_secs(8));
    }
    app.sync();
    let mut last: Option<Canvas> = None;
    let mut wipe: Option<Wipe> = None;
    let mut dirty = true;

    'main: loop {
        if dirty {
            let size = terminal.size()?;
            let mut frame = Canvas::new(size.width, size.height);
            ui::render(&mut app, &mut frame);
            if let Some(w) = &wipe {
                if !w.apply(&mut frame) {
                    wipe = None;
                }
            }
            terminal.draw(|f| frame.flush_into(f.buffer_mut()))?;
            last = Some(frame);
            dirty = false;
        }

        // animate only while something moves; otherwise sleep until input
        // (or until a status-bar notice is due to disappear)
        let tick = if wipe.is_some() {
            Some(Duration::from_millis(16))
        } else if app.spinners_active() {
            Some(spinner::INTERVAL)
        } else {
            app.notice_deadline().map(|t| t.saturating_duration_since(std::time::Instant::now()))
        };
        let first = match tick {
            Some(d) => tokio::select! {
                m = rx.recv() => m,
                _ = tokio::time::sleep(d) => None,
            },
            None => rx.recv().await,
        };
        // a tick (no message) means an animation frame is due
        dirty |= first.is_none();
        let mut pending: Vec<Msg> = first.into_iter().collect();
        while let Ok(m) = rx.try_recv() {
            pending.push(m);
        }
        for m in pending {
            match app.handle(m) {
                Action::Quit => break 'main,
                Action::Unchanged => continue,
                // snapshot the OLD-theme frame before the next render
                Action::ThemeWipe(accent) => {
                    if let Some(snap) = last.take() {
                        wipe = Some(Wipe::new(snap, accent));
                    }
                }
                Action::None => {}
            }
            dirty = true;
            app.sync();
        }
    }

    if mouse {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    // in-flight requests don't need a graceful goodbye
    std::process::exit(0);
}
