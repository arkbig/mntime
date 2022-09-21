use std::io::Read;

pub fn run() -> proc_exit::ExitResult {
    let _args = crate::cli_args::parse();

    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        finalize_cli();
        default_panic_hook(panic_info);
    }));
    struct CliFinalizer;
    impl Drop for CliFinalizer {
        fn drop(&mut self) {
            finalize_cli();
        }
    }
    let _cli_finalizer = CliFinalizer;
    initialize_cli();

    let backend = tui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = crate::terminal::Wrapper::new(backend);

    let (tx, rx) = std::sync::mpsc::channel();

    let tick_rate = std::time::Duration::from_millis(100);
    let app = App::new();
    // If proc_exit::Exit had implemented Send, it could have returned it as is...
    let thread: std::thread::JoinHandle<(proc_exit::Code, Option<String>)> =
        std::thread::Builder::new()
            .name("App".to_string())
            .spawn(move || run_app(rx, &mut terminal, app, tick_rate))
            .unwrap();

    while !thread.is_finished() {
        if crossterm::event::poll(tick_rate).unwrap() {
            if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                use crossterm::event::{KeyCode, KeyModifiers};
                match (key.code, key.modifiers) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL)
                    | (KeyCode::Char('q'), KeyModifiers::NONE) => tx.send(Msg::Quit).unwrap(),
                    (KeyCode::Char('t'), KeyModifiers::ALT) => tx.send(Msg::TODO).unwrap(),
                    _ => {}
                }
            }
        }
    }

    let ret = thread.join().unwrap();
    if ret.0 == proc_exit::Code::SUCCESS && None == ret.1 {
        Ok(())
    } else {
        let res = proc_exit::Exit::new(ret.0);
        if let Some(msg) = ret.1 {
            Err(res.with_message(msg))
        } else {
            Err(res)
        }
    }
}

/// Initialize cli environment. Be sure to call finalize_cli.
fn initialize_cli() {
    crossterm::terminal::enable_raw_mode().unwrap();
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture).unwrap();
}

/// Finalize cli environment. Be sure to call this after initialize_cli.
fn finalize_cli() {
    if let Err(err) = crossterm::terminal::disable_raw_mode() {
        eprintln!("[ERROR] {}", err);
    }
    if let Err(err) = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)
    {
        eprintln!("[ERROR] {}", err);
    }
    println!();
}

//TODO: Turn off warning suppression later.
#[allow(clippy::upper_case_acronyms)]
enum Msg {
    Quit,
    TODO,
}

struct App {
    current: u16,
    progress: u16,
    throbber_state: throbber_widgets_tui::ThrobberState,
}

impl App {
    fn new() -> App {
        App {
            current: 0,
            progress: 0,
            throbber_state: throbber_widgets_tui::ThrobberState::default(),
        }
    }

    fn on_tick(&mut self) {
        self.progress += 1;
        if self.progress > 100 {
            self.progress = 0;
        }
        self.throbber_state.calc_next();
    }
}

fn run_app<B>(
    rx: std::sync::mpsc::Receiver<Msg>,
    terminal: &mut crate::terminal::Wrapper<B>,
    mut app: App,
    tick_rate: std::time::Duration,
) -> (proc_exit::Code, Option<String>)
where
    B: tui::backend::Backend,
{
    let mut last_tick = std::time::Instant::now();
    let cur = terminal.get_cursor();
    let mut cursor_y = cur.1;
    let mut time_child = std::process::Command::new("sh")
        .args(["-c", "gtime sleep 1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    loop {
        if app.current == 0 {
            app.current += 1;
            crossterm::execute!(
                std::io::stdout(),
                crossterm::style::SetAttribute(crossterm::style::Attribute::Bold),
                crossterm::style::Print(format!("Benchmark #{}", app.current)),
                crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
            )
            .unwrap();
            cursor_y += 1;
        }
        terminal.draw_if_tty(|f| ui(f, &mut cursor_y, &mut app));

        match time_child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let mut out = String::new();
                    time_child.stdout.unwrap().read_to_string(&mut out).unwrap();
                    let mut err = String::new();
                    time_child.stderr.unwrap().read_to_string(&mut err).unwrap();
                    println!("\r\nstdout={:?}\r\nstderr={:?}", out, err);
                    return (proc_exit::Code::SUCCESS, None);
                } else {
                    let mut err = String::new();
                    time_child.stderr.unwrap().read_to_string(&mut err).unwrap();
                    return (proc_exit::Code::new(status.code().unwrap()), Some(err));
                }
            }
            Ok(None) => {}
            Err(e) => {
                let err = format!("{:?}", e);
                return (proc_exit::Code::FAILURE, Some(err));
            }
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        let msg = rx.recv_timeout(timeout);
        match msg {
            Ok(Msg::Quit) => {
                time_child.kill().unwrap();
                return (proc_exit::Code::SUCCESS, None);
            }
            Ok(Msg::TODO) => {}
            _ => {}
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = std::time::Instant::now();
            // if app.progress == 20 {
            //     return Ok(());
            // }
        }
    }
}

fn ui<B: tui::backend::Backend>(f: &mut tui::Frame<B>, cursor_y: &mut u16, app: &mut App) {
    let height = 1;
    let size = f.size();
    while size.height < *cursor_y + height {
        println!();
        *cursor_y -= 1;
    }

    let rect = tui::layout::Rect::new(0, *cursor_y, size.width, height);
    let chunks = tui::layout::Layout::default()
        .direction(tui::layout::Direction::Horizontal)
        .constraints(
            [
                tui::layout::Constraint::Length(20),
                tui::layout::Constraint::Percentage(100),
            ]
            .as_ref(),
        )
        .split(rect);

    // let text = vec![
    //     Spans::from(vec![
    //         Span::raw("Running..."),
    //     ])
    // ];
    // let paragraph = Paragraph::new(text);
    // f.render_widget(paragraph, chunks[0]);
    let full = throbber_widgets_tui::Throbber::default()
        .label("Running...")
        .style(tui::style::Style::default().fg(tui::style::Color::Cyan))
        .throbber_style(
            tui::style::Style::default()
                .fg(tui::style::Color::Red)
                .add_modifier(tui::style::Modifier::BOLD),
        )
        .throbber_set(throbber_widgets_tui::ARROW)
        .use_type(throbber_widgets_tui::WhichUse::Spin);
    f.render_stateful_widget(full, chunks[0], &mut app.throbber_state);

    let label = format!("{} / 100", app.progress);
    let gauge = tui::widgets::Gauge::default()
        .gauge_style(tui::style::Style::default().fg(tui::style::Color::White))
        .percent(app.progress)
        .label(label);
    f.render_widget(gauge, chunks[1]);
}
