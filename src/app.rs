// Copyright © ArkBig
//! This file provides application flow.

use std::{cell::RefCell, collections::HashMap, rc::Rc};
use strum::IntoEnumIterator as _;

/// The application is started and terminated.
///
/// Runs on 3 threads, including itself.
/// Spawn two threads for updating and drawing the application.
/// - main thread (this): Input monitoring.
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
        let draw_tx_clone = draw_tx.clone();
        let updating_thread = s.spawn(|| {
            run_app(
                update_rx,
                update_tick_rate,
                draw_tx_clone,
                model.clone(),
                &cli_args,
            )
        });
        let drawing_thread = s.spawn(|| {
            view_app(
                draw_rx,
                draw_tick_rate,
                model.clone(),
                &cli_args,
                &mut terminal,
            )
        });

        // Input monitoring.
        let is_in_tty = atty::is(atty::Stream::Stdin);
        while !updating_thread.is_finished() {
            if is_in_tty && crossterm::event::poll(update_tick_rate).unwrap() {
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
    let exit_code = ret.0;
    let exit_msg = ret.1;
    if exit_code == proc_exit::Code::SUCCESS && exit_msg.is_none() {
        Ok(())
    } else {
        let res = proc_exit::Exit::new(exit_code);
        if let Some(msg) = exit_msg {
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
fn initialize_cli() -> Option<CliFinalizer> {
    if atty::is(atty::Stream::Stdout) && atty::is(atty::Stream::Stderr) {
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

        Some(_cli_finalizer)
    } else {
        None
    }
}

/// Finalize cli environment.
///
/// It is set by initialize_cli() to be called automatically.
///
/// It can be called in duplicate, and even if some errors occur,
/// all termination processing is performed anyway.
fn finalize_cli() {
    if atty::is(atty::Stream::Stdout) && atty::is(atty::Stream::Stderr) {
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
    current_reports: Vec<HashMap<crate::cmd::MeasItem, f64>>,
}

/// Updating thread job
fn run_app(
    rx: std::sync::mpsc::Receiver<UpdateMsg>,
    tick_rate: std::time::Duration,
    draw_tx: std::sync::mpsc::Sender<DrawMsg>,
    model: std::sync::Arc<std::sync::RwLock<SharedViewModel>>,
    cli_args: &crate::cli_args::CliArgs,
) -> (proc_exit::Code, Option<String>) {
    // Checking available
    let time_commands = prepare_time_commands(&rx, tick_rate, cli_args);
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
    if !cli_args.use_builtin_only {
        if !cli_args.no_bsd
            && !time_commands
                .iter()
                .any(|x| x.borrow().cmd_type == crate::cmd::CmdType::Bsd)
        {
            draw_tx.send(DrawMsg::Warn("The bsd time command not found. Please install or specify `--no-bsd` to turn off this warning.".to_string())).unwrap();
        }
        if !cli_args.no_gnu
            && !time_commands
                .iter()
                .any(|x| x.borrow().cmd_type == crate::cmd::CmdType::Gnu)
        {
            draw_tx.send(DrawMsg::Warn("The gnu time command not found. Please install or specify `--no-gnu=` to turn off this warning.".to_string())).unwrap();
        }
    }

    // Benchmarking
    let mut last_tick = std::time::Instant::now();
    for (target_index, target) in cli_args.normalized_commands().iter().enumerate() {
        draw_tx
            .send(DrawMsg::PrintH(format!(
                "Benchmark #{}> {}",
                target_index + 1,
                target
            )))
            .unwrap();
        {
            let mut m = model.write().unwrap();
            m.current_reports = Vec::new();
            m.current_max = cli_args.runs;
            draw_tx.send(DrawMsg::StartMeasure).unwrap();
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
                    let time_cmd_result = if cli_args.loops <= 1 {
                        (*time_cmd).borrow_mut().execute(target.as_str())
                    } else {
                        (*time_cmd).borrow_mut().execute(
                            format!(
                                "sh -c 'for i in {} ;do {};done'",
                                vec!["0"; cli_args.loops as usize].join(" "),
                                target
                            )
                            .as_str(),
                        )
                    };
                    if let Err(err) = time_cmd_result {
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
        draw_tx
            .send(DrawMsg::ReportMeasure(
                model.read().unwrap().current_reports.clone(),
            ))
            .unwrap();
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
    matches!(msg, Ok(UpdateMsg::Quit))
}

/// Checks and returns the time command to be used.
///
/// The default is to try to run BSD and GNU alternately.
/// If neither of those is available, use built-in.
fn prepare_time_commands(
    rx: &std::sync::mpsc::Receiver<UpdateMsg>,
    tick_rate: std::time::Duration,
    cli_args: &crate::cli_args::CliArgs,
) -> Option<Vec<Rc<RefCell<crate::cmd::TimeCmd>>>> {
    let mut commands = Vec::<_>::new();
    if !cli_args.use_builtin_only {
        if !cli_args.no_bsd {
            let mut fallback_sh = false;
            loop {
                let mut cmd = crate::cmd::try_new_bsd_time(cli_args, fallback_sh);
                match command_available(rx, tick_rate, &mut cmd) {
                    None => return None,
                    Some(available) => {
                        if available {
                            commands.push(Rc::new(RefCell::new(cmd.unwrap())));
                            break;
                        } else if fallback_sh {
                            break;
                        } else {
                            fallback_sh = true;
                        }
                    }
                }
            }
        }
        if !cli_args.no_gnu {
            let mut fallback_sh = false;
            let mut fallback_time = false;
            loop {
                let mut cmd = crate::cmd::try_new_gnu_time(cli_args, fallback_sh, fallback_time);
                match command_available(rx, tick_rate, &mut cmd) {
                    None => return None,
                    Some(available) => {
                        if available {
                            commands.push(Rc::new(RefCell::new(cmd.unwrap())));
                            break;
                        } else if fallback_sh && fallback_time {
                            break;
                        } else if fallback_sh {
                            fallback_time = true;
                        } else if fallback_time {
                            fallback_sh = true;
                            fallback_time = false;
                        } else {
                            fallback_time = true;
                        }
                    }
                }
            }
        }
    }
    if commands.is_empty() {
        let mut fallback_sh = false;
        loop {
            let mut cmd = crate::cmd::try_new_builtin_time(cli_args, fallback_sh);
            match command_available(rx, tick_rate, &mut cmd) {
                None => return None,
                Some(available) => {
                    if available {
                        commands.push(Rc::new(RefCell::new(cmd.unwrap())));
                        break;
                    } else if fallback_sh {
                        break;
                    } else {
                        fallback_sh = true;
                    }
                }
            }
        }
    }
    Some(commands)
}

/// Check if the specified time command is available.
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

/// Messages received by drawing thread.
enum DrawMsg {
    Quit,
    Warn(String),
    PrintH(String),
    StartMeasure,
    ReportMeasure(Vec<HashMap<crate::cmd::MeasItem, f64>>),
}

// Drawing thread state.
#[derive(Default, Debug)]
struct DrawState {
    measuring: bool,
    throbber: throbber_widgets_tui::ThrobberState,
}

// Drawing thread job
fn view_app<B>(
    rx: std::sync::mpsc::Receiver<DrawMsg>,
    tick_rate: std::time::Duration,
    model: std::sync::Arc<std::sync::RwLock<SharedViewModel>>,
    cli_args: &crate::cli_args::CliArgs,
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
            Ok(DrawMsg::Warn(text)) => {
                terminal.clear_after();
                terminal.queue_attribute_err(crossterm::style::Attribute::Bold);
                terminal.queue_fg_err(crossterm::style::Color::Yellow);
                terminal
                    .queue_print_err(crossterm::style::Print(format!("[WARNING]: {0}\r\n", text)));
                terminal.flush_err(true);
            }
            Ok(DrawMsg::PrintH(text)) => {
                terminal.clear_after();
                static CONTINUE_TIME: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if CONTINUE_TIME.load(std::sync::atomic::Ordering::Relaxed) {
                    terminal.queue_print(crossterm::style::Print("\r\n"));
                }
                terminal.queue_attribute(crossterm::style::Attribute::Bold);
                terminal.queue_fg(crossterm::style::Color::Cyan);
                terminal.queue_print(crossterm::style::Print(text + "\r\n"));
                terminal.flush(true);
                CONTINUE_TIME.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Ok(DrawMsg::StartMeasure) => {
                draw_state.measuring = true;
            }
            Ok(DrawMsg::ReportMeasure(reports)) => {
                draw_state.measuring = false;
                terminal.clear_after();
                print_reports(terminal, reports.as_ref(), cli_args.loops);
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
                    cli_args.loops,
                )
            });
            last_tick = std::time::Instant::now();
            terminal.set_cursor(0, cur_y);
            draw_state.throbber.calc_next();
        }
    }
}

/// Draw loop.
fn ui<B>(
    f: &mut tui::Frame<B>,
    model: &SharedViewModel,
    state: &mut DrawState,
    cur_y: &mut u16,
    loops: u16,
) where
    B: tui::backend::Backend,
{
    let mut _offset_y = 0;
    if state.measuring {
        _offset_y += draw_progress(f, model, state, cur_y, _offset_y, loops);
        _offset_y += draw_summary_report(f, model, state, cur_y, _offset_y, loops);
    }
}

fn draw_progress<B>(
    f: &mut tui::Frame<B>,
    model: &SharedViewModel,
    state: &mut DrawState,
    cur_y: &mut u16,
    offset_y: u16,
    loops: u16,
) -> u16
where
    B: tui::backend::Backend,
{
    let size = f.size();
    let height = 1;
    if size.height < offset_y + height {
        return 0;
    }
    while size.height < *cur_y + offset_y + height {
        println!();
        *cur_y -= 1;
    }

    let rect = tui::layout::Rect::new(0, *cur_y + offset_y, size.width, height);
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
            .copied()
            .collect();
        let stats = crate::stats::Stats::new(&samples);
        if 0.0 < stats.mean {
            format!(
                "Mean {}, so about {} left",
                crate::cmd::meas_item_unit_value(&crate::cmd::MeasItem::Real, stats.mean, loops),
                crate::cmd::meas_item_unit_value(
                    &crate::cmd::MeasItem::Real,
                    stats.mean * ((model.current_max - model.current_run) as f64),
                    1 // Actual time not divided by loops.
                )
            )
        } else {
            String::from("Measuring...")
        }
    };
    let gauge = tui::widgets::Gauge::default()
        .gauge_style(tui::style::Style::default().fg(tui::style::Color::Cyan))
        .ratio(model.current_run as f64 / model.current_max as f64)
        .label(label);
    f.render_widget(gauge, chunks[1]);

    height
}

fn draw_summary_report<B>(
    f: &mut tui::Frame<B>,
    model: &SharedViewModel,
    _state: &mut DrawState,
    cur_y: &mut u16,
    offset_y: u16,
    loops: u16,
) -> u16
where
    B: tui::backend::Backend,
{
    use crate::cmd::{meas_item_unit_value, MeasItem};

    let size = f.size();
    let height = 1;
    if size.height < offset_y + height {
        return 0;
    }
    while size.height < *cur_y + offset_y + height {
        println!();
        *cur_y -= 1;
    }

    let rect = tui::layout::Rect::new(0, *cur_y + offset_y, size.width, height);
    let chunks = tui::layout::Layout::default()
        .direction(tui::layout::Direction::Horizontal)
        .constraints(
            [
                tui::layout::Constraint::Percentage(33),
                tui::layout::Constraint::Percentage(33),
                tui::layout::Constraint::Percentage(33),
            ]
            .as_ref(),
        )
        .split(rect);

    for (index, item) in vec![MeasItem::Real, MeasItem::User, MeasItem::Sys]
        .iter()
        .enumerate()
    {
        let samples: Vec<_> = model
            .current_reports
            .iter()
            .filter_map(|x| x.get(item))
            .copied()
            .collect();
        let stats = crate::stats::Stats::new(&samples);
        let text = tui::widgets::Paragraph::new(tui::text::Spans::from(format!(
            "{} {} ± {}",
            item.as_ref(),
            meas_item_unit_value(item, stats.mean, loops),
            meas_item_unit_value(item, stats.stdev, loops),
        )));
        f.render_widget(text, chunks[index]);
    }

    height
}

fn print_reports<B>(
    terminal: &mut crate::terminal::Wrapper<B>,
    reports: &[HashMap<crate::cmd::MeasItem, f64>],
    loops: u16,
) where
    B: tui::backend::Backend,
{
    use crate::cmd::{meas_item_name, meas_item_name_max_width, meas_item_unit_value};

    const MEAN_WIDTH: usize = 13;

    let mut lines = Vec::new();
    let mut exist_error = false;
    for item in crate::cmd::MeasItem::iter() {
        let samples: Vec<_> = reports
            .iter()
            .filter_map(|x| x.get(&item))
            .copied()
            .collect();
        match item {
            crate::cmd::MeasItem::Real | crate::cmd::MeasItem::User | crate::cmd::MeasItem::Sys => {
                // Required.
            }
            _ => {
                // Skip if can't measure.
                if !samples.iter().any(|&x| x.to_bits() != 0) {
                    continue;
                }
                if samples.is_empty() {
                    continue;
                }
            }
        }
        if item == crate::cmd::MeasItem::ExitStatus {
            exist_error = true;
            print_exit_status(terminal, &samples, loops);
            continue;
        }
        let stats = crate::stats::Stats::new(&samples);
        lines.push(format!(
            "{:name_width$}:{:>mean_width$} ± {} ({:.1} %) [{} ≦ {} ≦ {}] / {}",
            meas_item_name(&item, loops),
            meas_item_unit_value(&item, stats.mean, loops),
            meas_item_unit_value(&item, stats.stdev, loops),
            stats.calc_cv() * 100.0,
            meas_item_unit_value(&item, stats.min(), loops),
            meas_item_unit_value(&item, stats.median(), loops),
            meas_item_unit_value(&item, stats.max(), loops),
            stats.count(),
            name_width = meas_item_name_max_width(loops),
            mean_width = MEAN_WIDTH,
        ));
        if stats.has_outlier() {
            lines.push(format!(
                "{:^name_width$}:{:>mean_width$} ± {} ({:.1} %) [{} ≦ {} ≦ {}] / {}(-{})",
                "└─Excluding Outlier",
                meas_item_unit_value(&item, stats.mean_excluding_outlier, loops),
                meas_item_unit_value(&item, stats.stdev_excluding_outlier, loops),
                stats.calc_cv_excluding_outlier() * 100.0,
                meas_item_unit_value(&item, stats.min_excluding_outlier(), loops),
                meas_item_unit_value(&item, stats.median_excluding_outlier(), loops),
                meas_item_unit_value(&item, stats.max_excluding_outlier(), loops),
                stats.count_excluding_outlier(),
                stats.outlier_count,
                name_width = meas_item_name_max_width(loops),
                mean_width = MEAN_WIDTH,
            ));
        }
    }

    if exist_error {
        terminal.queue_fg(crossterm::style::Color::Red);
    } else {
        terminal.queue_fg(crossterm::style::Color::Green);
    }
    terminal.queue_print(crossterm::style::Print(format!(
        "{:^name_width$}:{:>mean_width$} ± σ (Coefficient of variation %) [Min ≦ Median ≦ Max] / Valid count\r\n",
        "LEGEND",
        "Mean",
        name_width = meas_item_name_max_width(loops),
        mean_width = MEAN_WIDTH,
    )));
    terminal.queue_attribute(crossterm::style::Attribute::Reset);

    terminal.queue_print(crossterm::style::Print(lines.join("\r\n") + "\r\n"));
    terminal.flush(true);
}

fn print_exit_status<B>(terminal: &mut crate::terminal::Wrapper<B>, samples: &Vec<f64>, loops: u16)
where
    B: tui::backend::Backend,
{
    use crate::cmd::{meas_item_name, meas_item_name_max_width};

    let mut histogram = samples.iter().fold(HashMap::<i32, i16>::new(), |mut s, x| {
        let code = x.floor() as i32;
        if let std::collections::hash_map::Entry::Vacant(e) = s.entry(code) {
            e.insert(1);
        } else {
            *s.get_mut(&code).unwrap() += 1;
        }
        s
    });
    let success = *histogram.get(&0).unwrap_or(&0);
    if histogram.get(&0).is_some() {
        histogram.remove(&0);
    }
    let failure = samples.len() - success as usize;
    let mut failure_codes = histogram.iter().collect::<Vec<_>>();
    failure_codes.sort_by(|a, b| a.0.cmp(b.0));
    terminal.queue_fg(crossterm::style::Color::Red);
    terminal.queue_print(crossterm::style::Print(format!(
        "{:>name_width$}: ",
        meas_item_name(&crate::cmd::MeasItem::ExitStatus, loops),
        name_width = meas_item_name_max_width(loops)
    )));
    terminal.queue_fg(crossterm::style::Color::Green);
    terminal.queue_print(crossterm::style::Print(format!(
        "Success {} times. ",
        success
    )));
    terminal.queue_fg(crossterm::style::Color::Red);
    terminal.queue_print(crossterm::style::Print(format!(
        "Failure {} times. [(code× times) {}]\r\n",
        failure,
        failure_codes
            .iter()
            .map(|x| format!("{}× {}", x.0, x.1))
            .collect::<Vec<_>>()
            .join(", ")
    )));
    terminal.queue_attribute(crossterm::style::Attribute::Reset);
    terminal.flush(true);
}
