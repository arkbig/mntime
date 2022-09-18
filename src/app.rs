use clap::Parser;

/// Command Line Arguments
#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None, setting = clap::builder::AppSettings::TrailingVarArg | clap::builder::AppSettings::DeriveDisplayOrder)]
struct CliArgs {
    /// Perform exactly NUM runs for each command.
    #[clap(short, long, value_parser, value_name = "NUM", default_value_t = 5)]
    runs: u16,

    /// Time command used.
    #[clap(
        short = 'T',
        long,
        value_parser,
        value_name = "COMMAND",
        default_value = "gtime"
    )]
    time_command: String,

    /// Arguments of the time command used.
    ///
    /// Quoting if flag is included or there are multiple args.
    #[clap(short, long, value_parser, value_name = "ARGS", default_value = "")]
    time_args: String,

    /// The commands to benchmark.
    ///
    /// If multiple commands are specified, each is executed and compared.
    /// One command is specified with "--" delimiters (recommended) or quotation.
    /// However, in the case of command-only quotation marks,
    /// the subsequent ones are considered to be the arguments of the command.
    ///
    /// e.g.) mntime command1 --flag arg -- command2 -- 'command3 -f -- args'
    #[clap(value_parser)]
    commands: Vec<String>,
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

pub fn run() -> proc_exit::ExitResult {
    // let args = CliArgs::parse();

    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        finalize_cli();
        default_panic_hook(panic_info);
    }));
    initialize_cli();

    let backend = tui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = tui::Terminal::new(backend).unwrap();

    // create app and run it
    let tick_rate = std::time::Duration::from_millis(100);
    let app = App::new();
    let res = run_app(&mut terminal, app, tick_rate);

    finalize_cli();
    res
}

fn run_app<B: tui::backend::Backend>(
    terminal: &mut tui::Terminal<B>,
    mut app: App,
    tick_rate: std::time::Duration,
) -> proc_exit::ExitResult {
    let is_in_tty = atty::is(atty::Stream::Stdin);
    let is_io_tty = is_in_tty && atty::is(atty::Stream::Stdout);
    let mut last_tick = std::time::Instant::now();
    let cur = if is_io_tty {
        terminal.get_cursor().unwrap()
    } else {
        (0, 0)
    };
    let mut cursor_y = cur.1;
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
        if is_io_tty {
            terminal.draw(|f| ui(f, &mut cursor_y, &mut app)).unwrap();
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        if is_in_tty && crossterm::event::poll(timeout).unwrap() {
            if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                if let crossterm::event::KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        } else {
            std::thread::sleep(timeout);
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = std::time::Instant::now();
            if app.progress == 20 {
                return Ok(());
            }
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
