use std::{borrow::BorrowMut, cell::RefCell, rc::Rc};
use strum::IntoEnumIterator;

/// The application is started and terminated.
///
/// Spawn two threads for updating and drawing the application.
/// - main thread: Input monitoring.
/// - updating thread: Business logic processing and updating data for drawing.
/// - drawing thread: Output process.
pub fn run() -> proc_exit::ExitResult {
    let cli_args = crate::cli_args::parse();

    let _cli_finalizer = initialize_cli();

    // for updating thread
    let (update_tx, update_rx) = std::sync::mpsc::channel();
    let update_tick_rate = std::time::Duration::from_millis(50);
    let model = std::sync::Arc::new(std::sync::RwLock::new(SharedViewModel::default()));
    // for drawing thread
    let (draw_tx, draw_rx) = std::sync::mpsc::channel();
    let draw_tick_rate = std::time::Duration::from_millis(100);
    let backend = tui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = crate::terminal::Wrapper::new(backend);

    let mut ret = (proc_exit::Code::SUCCESS, None);
    std::thread::scope(|s| {
        // Spawn threads
        let draw_tx_clone = draw_tx.clone();
        let updating_thread = s.spawn(|| {
            run_app(
                update_rx,
                update_tick_rate,
                draw_tx_clone,
                model.clone(),
                cli_args,
            )
        });
        let drawing_thread =
            s.spawn(|| view_app(draw_rx, draw_tick_rate, model.clone(), &mut terminal));

        // Input monitoring.
        while !updating_thread.is_finished() {
            if crossterm::event::poll(update_tick_rate).unwrap() {
                if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                    use crossterm::event::{KeyCode, KeyModifiers};
                    match (key.code, key.modifiers) {
                        // Cancellation.
                        (KeyCode::Char('c'), KeyModifiers::CONTROL)
                        | (KeyCode::Char('q'), KeyModifiers::NONE) => {
                            update_tx.send(UpdateMsg::Quit).unwrap()
                        }
                        _ => {}
                    }
                }
            }
        }

        // Terminated.
        draw_tx.send(DrawMsg::Quit).unwrap();
        drawing_thread.join().unwrap();
        ret = updating_thread.join().unwrap();
    });

    // Exit Code
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

struct CliFinalizer;
impl Drop for CliFinalizer {
    fn drop(&mut self) {
        finalize_cli();
    }
}

/// Initialize cli environment.
///
/// This returns a CliFinalizer that implements Drop, so please be good.
fn initialize_cli() -> CliFinalizer {
    // Automatic finalizer setup
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // stdout is disrupted, so the finalize first.
        finalize_cli();
        default_panic_hook(panic_info);
    }));
    let _cli_finalizer = CliFinalizer;

    crossterm::terminal::enable_raw_mode().unwrap();
    //crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture).unwrap();

    _cli_finalizer
}

/// Finalize cli environment.
///
/// It is set by initialize_cli() to be called automatically.
///
/// It can be called in duplicate, and even if some errors occur,
/// all termination processing is performed anyway.
fn finalize_cli() {
    // if let Err(err) = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)
    // {
    //     eprintln!("[ERROR] {}", err);
    // }

    if let Err(err) = crossterm::terminal::disable_raw_mode() {
        eprintln!("[ERROR] {}", err);
    }
    // Instead of resetting the cursor position.
    println!();
}

//=============================================================================
// Updating
//=============================================================================

/// Messages received by updating thread.
enum UpdateMsg {
    Quit,
}

/// Data model to be updated in the updating thread and viewed in the drawing thread.
#[derive(Default)]
struct SharedViewModel {
    current_run: u16,
    current_max: u16,
    current_reports: Vec<std::collections::HashMap<crate::cmd::MeasItem, f64>>,
}

/// Updating thread job
fn run_app(
    rx: std::sync::mpsc::Receiver<UpdateMsg>,
    tick_rate: std::time::Duration,
    draw_tx: std::sync::mpsc::Sender<DrawMsg>,
    model: std::sync::Arc<std::sync::RwLock<SharedViewModel>>,
    cli_args: crate::cli_args::CliArgs,
) -> (proc_exit::Code, Option<String>) {
    // Checking available
    let time_commands = prepare_time_commands(&rx, tick_rate);
    if time_commands.is_none() {
        // quit
        return (proc_exit::Code::FAILURE, None);
    }
    let time_commands = time_commands.unwrap();
    if time_commands.is_empty() {
        return (
            proc_exit::Code::FAILURE,
            Some(String::from(
                "time command not found. Install the BSD or GNU version or both.",
            )),
        );
    }

    // Benchmarking
    let mut last_tick = std::time::Instant::now();
    for target in cli_args.normalized_commands() {
        draw_tx
            .send(DrawMsg::PrintH(format!("Benchmark: {}", target)))
            .unwrap();
        {
            let mut m = model.write().unwrap();
            m.current_reports = Vec::new();
            m.current_max = cli_args.runs;
        }
        for n in 0..cli_args.runs {
            model.write().unwrap().current_run = n;
            let time_cmd = Rc::clone(&time_commands[(n as usize) % time_commands.len()]);
            let mut running = false;
            loop {
                if running {
                    if (*time_cmd).borrow_mut().is_finished() {
                        model
                            .write()
                            .unwrap()
                            .current_reports
                            .push((*time_cmd).borrow_mut().get_report().unwrap().clone());
                        break;
                    }
                } else {
                    if let Err(err) = (*time_cmd).borrow_mut().execute(target.as_str()) {
                        return (proc_exit::Code::FAILURE, Some(format!("{:}", err)));
                    }
                    running = true;
                }
                if wait_recv_quit(&rx, tick_rate, last_tick) {
                    if running {
                        (*time_cmd).borrow_mut().kill().unwrap();
                    }
                    return (proc_exit::Code::FAILURE, None);
                }
                last_tick = std::time::Instant::now();
            }
        }
        model.write().unwrap().current_max = 0;
        draw_tx.send(DrawMsg::FinalizeReport).unwrap();
    }
    (proc_exit::Code::SUCCESS, None)
}

fn wait_recv_quit(
    rx: &std::sync::mpsc::Receiver<UpdateMsg>,
    tick_rate: std::time::Duration,
    last_tick: std::time::Instant,
) -> bool {
    let timeout = tick_rate
        .checked_sub(last_tick.elapsed())
        .unwrap_or_else(|| std::time::Duration::from_secs(0));
    let msg = rx.recv_timeout(timeout);
    match msg {
        Ok(UpdateMsg::Quit) => true,
        _ => false,
    }
}

fn prepare_time_commands(
    rx: &std::sync::mpsc::Receiver<UpdateMsg>,
    tick_rate: std::time::Duration,
) -> Option<Vec<Rc<RefCell<crate::cmd::TimeCmd>>>> {
    let mut commands = Vec::<_>::new();
    let mut cmd = crate::cmd::try_new_bsd_time();
    match command_available(rx, tick_rate, &mut cmd) {
        None => return None,
        Some(available) => {
            if available {
                commands.push(Rc::new(RefCell::new(cmd.unwrap())));
            }
        }
    }
    let mut is_alias = true;
    loop {
        let mut cmd = crate::cmd::try_new_gnu_time(is_alias);
        match command_available(rx, tick_rate, &mut cmd) {
            None => return None,
            Some(available) => {
                if available {
                    commands.push(Rc::new(RefCell::new(cmd.unwrap())));
                    break;
                } else if is_alias {
                    // Retry a non-alias version.
                    is_alias = false;
                } else {
                    break;
                }
            }
        }
    }
    if commands.is_empty() {
        let mut cmd = crate::cmd::try_new_builtin_time();
        match command_available(rx, tick_rate, &mut cmd) {
            None => return None,
            Some(available) => {
                if available {
                    commands.push(Rc::new(RefCell::new(cmd.unwrap())));
                }
            }
        }
    }
    Some(commands)
}

fn command_available(
    rx: &std::sync::mpsc::Receiver<UpdateMsg>,
    tick_rate: std::time::Duration,
    command: &mut anyhow::Result<crate::cmd::TimeCmd>,
) -> Option<bool> {
    if command.is_err() {
        return Some(false);
    }
    let mut last_tick = std::time::Instant::now();
    let cmd = command.as_mut().unwrap();
    loop {
        match cmd.ready_status() {
            crate::cmd::ReadyStatus::Checking => {}
            crate::cmd::ReadyStatus::Ready => {
                return Some(true);
            }
            crate::cmd::ReadyStatus::Error => {
                return Some(false);
            }
        }

        if wait_recv_quit(rx, tick_rate, last_tick) {
            return None;
        }
        last_tick = std::time::Instant::now();
    }
}

//=============================================================================
// Drawing
//=============================================================================

enum DrawMsg {
    Quit,
    PrintH(String),
    FinalizeReport,
}

#[derive(Default, Debug)]
struct DrawState {
    throbber: throbber_widgets_tui::ThrobberState,
}

fn view_app<B>(
    rx: std::sync::mpsc::Receiver<DrawMsg>,
    tick_rate: std::time::Duration,
    model: std::sync::Arc<std::sync::RwLock<SharedViewModel>>,
    terminal: &mut crate::terminal::Wrapper<B>,
) where
    B: tui::backend::Backend,
{
    let mut draw_state = DrawState::default();

    let mut last_tick = std::time::Instant::now();
    loop {
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        let msg = rx.recv_timeout(timeout);
        match msg {
            Ok(DrawMsg::Quit) => {
                return;
            }
            Ok(DrawMsg::PrintH(text)) => {
                terminal.queue_attribute(crossterm::style::SetAttribute(
                    crossterm::style::Attribute::Bold,
                ));
                terminal.queue_print(crossterm::style::Print(text));
                terminal.flush(true);
            }
            Ok(DrawMsg::FinalizeReport) => {
                print_reports(terminal, &model.read().unwrap().current_reports);
            }
            _ => {}
        }

        if last_tick.elapsed() >= tick_rate {
            let mut cur_y = terminal.get_cursor().1;
            terminal.draw_if_tty(|f| {
                ui(
                    f,
                    model.read().as_ref().unwrap(),
                    &mut draw_state,
                    &mut cur_y,
                )
            });
            last_tick = std::time::Instant::now();
            draw_state.throbber.calc_next();
        }
    }
}

fn ui<B>(f: &mut tui::Frame<B>, model: &SharedViewModel, state: &mut DrawState, cur_y: &mut u16)
where
    B: tui::backend::Backend,
{
    let size = f.size();

    if 0 < model.current_max {
        draw_progress(f, model, state, &size, cur_y);
    }
}

fn draw_progress<B>(
    f: &mut tui::Frame<B>,
    model: &SharedViewModel,
    state: &mut DrawState,
    size: &tui::layout::Rect,
    cur_y: &mut u16,
) where
    B: tui::backend::Backend,
{
    let height = 1;
    if size.height < height {
        return;
    }
    while size.height < *cur_y + height {
        println!();
        *cur_y -= 1;
    }

    let rect = tui::layout::Rect::new(0, *cur_y, size.width, height);
    let chunks = tui::layout::Layout::default()
        .direction(tui::layout::Direction::Horizontal)
        .constraints(
            [
                tui::layout::Constraint::Min(10),
                tui::layout::Constraint::Percentage(100),
            ]
            .as_ref(),
        )
        .split(rect);

    let throbber = throbber_widgets_tui::Throbber::default()
        .label(format!("{:>3}/{:<3}", model.current_run, model.current_max))
        .style(tui::style::Style::default().fg(tui::style::Color::Cyan))
        .throbber_set(throbber_widgets_tui::CLOCK)
        .use_type(throbber_widgets_tui::WhichUse::Spin);
    f.render_stateful_widget(throbber, chunks[0], &mut state.throbber);

    let label = if model.current_reports.is_empty() {
        String::from("Measuring...")
    } else {
        let samples: Vec<_> = model
            .current_reports
            .iter()
            .filter_map(|x| x.get(&crate::cmd::MeasItem::Real))
            .map(|x| *x)
            .collect();
        let stats = crate::stats::Stats::new(&samples);
        if 0.0 < stats.mean {
            format!(
                "Mean {}, so about {} left",
                crate::cmd::meas_item_unit_value(&crate::cmd::MeasItem::Real, stats.mean),
                crate::cmd::meas_item_unit_value(
                    &crate::cmd::MeasItem::Real,
                    stats.mean * ((model.current_max - model.current_run) as f64)
                )
            )
        } else {
            String::from("Measuring...")
        }
    };
    let gauge = tui::widgets::Gauge::default()
        .gauge_style(tui::style::Style::default().fg(tui::style::Color::White))
        .ratio(model.current_run as f64 / model.current_max as f64)
        .label(label);
    f.render_widget(gauge, chunks[1]);
}

fn print_reports<B>(
    terminal: &mut crate::terminal::Wrapper<B>,
    reports: &Vec<std::collections::HashMap<crate::cmd::MeasItem, f64>>,
) where
    B: tui::backend::Backend,
{
    use crate::cmd::{meas_item_name, meas_item_name_max_width, meas_item_unit_value};
    let mut lines = vec![format!(
        "{:^width$}: Mean ± σ (Coefficient of variation %) [Min ≦ Median ≦ Max] / Valid count",
        "LEGEND",
        width = meas_item_name_max_width()
    )];
    for item in crate::cmd::MeasItem::iter() {
        let samples: Vec<_> = reports
            .iter()
            .filter_map(|x| x.get(&item))
            .map(|x| *x)
            .collect();
        if !samples.iter().any(|&x| x.to_bits() != 0) {
            continue;
        }
        if samples.len() == 0 {
            continue;
        }
        let stats = crate::stats::Stats::new(&samples);
        lines.push(format!(
            "{:width$}: {} ± {} ({:.1} %) [{} ≦ {} ≦ {}] / {}",
            meas_item_name(&item),
            meas_item_unit_value(&item, stats.mean),
            meas_item_unit_value(&item, stats.stdev),
            stats.calc_cv() * 100.0,
            meas_item_unit_value(&item, stats.min()),
            meas_item_unit_value(&item, stats.median()),
            meas_item_unit_value(&item, stats.max()),
            stats.valid_count(),
            width = meas_item_name_max_width()
        ));
        if stats.has_outlier() {
            lines.push(format!(
                "{:^width$}: {} ± {} ({:.1} %) [{} ≦ {} ≦ {}] / {}(+{})",
                "└─Excluding Outlier",
                meas_item_unit_value(&item, stats.mean_excluding_outlier),
                meas_item_unit_value(&item, stats.stdev_excluding_outlier),
                stats.calc_cv_excluding_outlier() * 100.0,
                meas_item_unit_value(&item, stats.min_excluding_outlier()),
                meas_item_unit_value(&item, stats.median_excluding_outlier()),
                meas_item_unit_value(&item, stats.max_excluding_outlier()),
                stats.count(),
                stats.outlier_count,
                width = meas_item_name_max_width()
            ));
        }
    }
}
