use clap::{AppSettings, Parser};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    style::{Print, Attribute, SetAttribute},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Gauge, Paragraph},
    Frame, Terminal,
};

/// Command Line Args
// TODO: I want to embed package.name in the about document.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, setting = AppSettings::TrailingVarArg)]
struct CliArgs {
    /// Perform exactly NUM runs for each command.
    #[clap(short, long, value_parser, value_name = "NUM", default_value_t = 5)]
    runs: u16,

    /// Time command used.
    #[clap(short='T', long, value_parser, value_name = "COMMAND", default_value = "gtime")]
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

struct App {
    current: u16,
    progress: u16,
}

impl App {
    fn new() -> App {
        App {
            current: 0,
            progress: 0,
        }
    }

    fn on_tick(&mut self) {
        self.progress += 1;
        if self.progress > 10 {
            self.progress = 0;
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(100);
    let app = App::new();
    let res = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let cur = terminal.get_cursor()?;
    let mut cursor_y = cur.1;

    loop {
        if app.current == 0 {
            app.current += 1;
            execute!(
                io::stdout(),
                SetAttribute(Attribute::Bold),
                Print(format!("Benchmark #{}", app.current)),
                SetAttribute(Attribute::Reset),
            );
            cursor_y += 1;
        }
        terminal.draw(|f| ui(f, &mut cursor_y, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
            if app.progress == 0 {
                return Ok(());
            }
        }
    }
    return Ok(());
}

fn ui<B: Backend>(f: &mut Frame<B>, cursor_y: &mut u16, app: &App) {
    let height = 1;
    let size = f.size();
    while size.height < *cursor_y + height {
        println!();
        *cursor_y -= 1;
    }

    let rect = Rect::new(0, *cursor_y, size.width, height);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(20),
                Constraint::Percentage(100),
            ]
            .as_ref(),
        )
        .split(rect);

    let text = vec![
        Spans::from(vec![
            Span::raw("Running..."),
        ])
    ];
    let paragraph = Paragraph::new(text);
    f.render_widget(paragraph, chunks[0]);

    let label = format!("{}/10", app.progress);
    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        )
        .percent(app.progress * 10)
        .label(label);
    f.render_widget(gauge, chunks[1]);
}
