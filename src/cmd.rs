use anyhow::Context;
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ReadyStatus {
    Checking,
    Ready,
    Error,
}

pub enum MeasUnit {
    Second,
    Percent,
    Byte,
    None,
}

///  Measurement Items
///
/// Those that can be measured by [time -l][time] & [gtime -v][gtime], excluding those unmaintained by [getrusage(2)][getrusage]
///
/// [time]:https://www.freebsd.org/cgi/man.cgi?query=time
/// [gtime]:https://man7.org/linux/man-pages/man1/time.1.html
/// [getrusage]:https://man7.org/linux/man-pages/man2/getrusage.2.html
#[derive(Debug, Hash, Eq, PartialEq)]
pub enum MeasItems {
    Real,
    User,
    Sys,
    CpuUsage,
    //(unmaintained) AvgSharedText,
    //(unmaintained) AvgUnsharedData,
    //(unmaintained) AvgStack,
    //maybe (unmaintained) AvgTotal,
    MaxResident,
    //maybe (unmaintained) AvgResident,
    MajorPageFault,
    MinorPageFault,
    VoluntaryCtxSwitch,
    InvoluntaryCtxSwitch,
    //(unmaintained) Swap,
    BlockInput,
    BlockOutput,
    //(unmaintained) MsgSend,
    //(unmaintained) MsgRecv,
    //(unmaintained) SignalRecv,
    Page,
    Instruction,
    Cycle,
    PeakMemory,
    Unknown(String),
}

pub fn meas_item_name(item: &MeasItems) -> &str {
    match item {
        MeasItems::Real => "Elapsed (wall clock) time",
        MeasItems::User => "User time",
        MeasItems::Sys => "System time",
        MeasItems::CpuUsage => "Percent of CPU this job got",
        //  MeasItems::AvgSharedText => "Average shared text size",
        //  MeasItems::AvgUnsharedData => "Average unshared data size",
        //  MeasItems::AvgStack => "Average stack size",
        //  MeasItems::AvgTotal => "Average total size",
        MeasItems::MaxResident => "Maximum resident set size",
        //  MeasItems::AvgResident => "Average resident set size",
        MeasItems::MajorPageFault => "Major (requiring I/O) page faults",
        MeasItems::MinorPageFault => "Minor (reclaiming a frame) page faults",
        MeasItems::VoluntaryCtxSwitch => "Voluntary context switches",
        MeasItems::InvoluntaryCtxSwitch => "Involuntary context switches",
        //  MeasItems::Swap => "Swaps",
        MeasItems::BlockInput => "File system inputs block",
        MeasItems::BlockOutput => "File system outputs block",
        //  MeasItems::MsgSend => "Socket messages sent",
        //  MeasItems::MsgRecv => "Socket messages received",
        //  MeasItems::SignalRecv => "Signals received",
        MeasItems::Page => "Page size",
        MeasItems::Instruction => "Instructions retired",
        MeasItems::Cycle => "Cycles elapsed",
        MeasItems::PeakMemory => "Peak memory footprint",
        MeasItems::Unknown(name) => name,
    }
}

pub fn meas_item_unit(item: &MeasItems) -> MeasUnit {
    match item {
        MeasItems::Real |  MeasItems::User |  MeasItems::Sys => MeasUnit::Second,
        MeasItems::CpuUsage => MeasUnit::Percent,
        MeasItems::MaxResident
        // |  MeasItems::AvgSharedText
        // |  MeasItems::AvgUnsharedData
        // |  MeasItems::AvgStack
        // |  MeasItems::AvgTotal
        // |  MeasItems::AvgResident
        |  MeasItems::PeakMemory => MeasUnit::Byte,
        MeasItems::MajorPageFault
        |  MeasItems::MinorPageFault
        |  MeasItems::VoluntaryCtxSwitch
        |  MeasItems::InvoluntaryCtxSwitch
        // |  MeasItems::Swap
        |  MeasItems::BlockInput
        |  MeasItems::BlockOutput
        // |  MeasItems::MsgSend
        // |  MeasItems::MsgRecv
        // |  MeasItems::SignalRecv
        |  MeasItems::Instruction
        |  MeasItems::Cycle
        |  MeasItems::Page
        |  MeasItems::Unknown(_) => MeasUnit::None,
    }
}

pub trait Cmd {
    fn ready_status(&mut self) -> ReadyStatus;
    fn execute(&mut self, command: &str) -> anyhow::Result<()>;
    fn is_finished(&mut self) -> bool;
    fn get_report(&mut self) -> anyhow::Result<&std::collections::HashMap<MeasItems, f64>>;
    fn kill(&mut self) -> anyhow::Result<()>;
}

#[derive(Error, Debug)]
enum CmdError {
    #[error("Execution command is not finished yet. This is a bug in the source code.")]
    NotFinished,
    #[error("Could not parse the output of the `{0}` command. This is a source code issue, please provide the developer with the output of the `{0}` command.")]
    ParseError(&'static str),
}

#[derive(Debug)]
pub struct BuiltinCmd {
    process: std::process::Child,
    ready_status: ReadyStatus,
    meas_report: Option<std::collections::HashMap<MeasItems, f64>>,
}

impl BuiltinCmd {
    pub fn try_new() -> anyhow::Result<Self> {
        // test to use
        Ok(Self {
            process: execute("sh", &["-c", "time true"])?,
            ready_status: ReadyStatus::Checking,
            meas_report: None,
        })
    }

    fn re_time(&self) -> &'static regex::Regex {
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| {
            regex::Regex::new(r"(?P<name>\w+)\t(?:(?P<min>\d+)m)?(?P<sec>[0-9.]+)s?").unwrap()
        })
    }
}

impl Cmd for BuiltinCmd {
    fn ready_status(&mut self) -> ReadyStatus {
        if self.ready_status == ReadyStatus::Checking {
            if self.is_finished() {
                let err_msg = stderr(&mut self.process);
                if self.re_time().is_match(err_msg.as_str()) {
                    self.ready_status = ReadyStatus::Ready;
                } else {
                    self.ready_status = ReadyStatus::Error;
                }
            }
        }
        self.ready_status
    }

    fn execute(&mut self, command: &str) -> anyhow::Result<()> {
        self.meas_report = None;
        self.process = execute("sh", &["-c", format!("time {}", command).as_str()])?;
        Ok(())
    }

    fn is_finished(&mut self) -> bool {
        self.process.try_wait().unwrap().is_some()
    }

    fn get_report(&mut self) -> anyhow::Result<&std::collections::HashMap<MeasItems, f64>> {
        anyhow::ensure!(self.is_finished(), CmdError::NotFinished);

        if self.meas_report.is_some() {
            return Ok(self.meas_report.as_ref().unwrap());
        }

        let mut meas_items = std::collections::HashMap::<MeasItems, f64>::new();
        let err_msg = stderr(&mut self.process);
        for cap in self.re_time().captures_iter(err_msg.as_str()) {
            let min: f64 = if let Some(min_match) = cap.name("min") {
                min_match.as_str().parse().unwrap()
            } else {
                0.0
            };
            let sec: f64 = (&cap["sec"]).parse().unwrap();
            let v = min * 60.0 + sec;
            match &cap["name"] {
                "real" => {
                    meas_items.insert(MeasItems::Real, v);
                }
                "user" => {
                    meas_items.insert(MeasItems::User, v);
                }
                "sys" => {
                    meas_items.insert(MeasItems::Sys, v);
                }
                _ => {
                    if v != 0.0 {
                        meas_items.insert(MeasItems::Unknown(String::from(&cap["name"])), v);
                    }
                }
            }
        }
        if meas_items.is_empty() {
            Err(CmdError::ParseError("time").into())
        } else {
            self.meas_report = Some(meas_items);
            Ok(self.meas_report.as_ref().unwrap())
        }
    }

    fn kill(&mut self) -> anyhow::Result<()> {
        self.process.kill().context("Could not kill time process.")
    }
}

fn execute(program: &str, args: &[&str]) -> anyhow::Result<std::process::Child> {
    std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "Could not start `{}` execution with argument `{}`",
                program,
                args.join(" ")
            )
        })
}

fn stderr(child: &mut std::process::Child) -> String {
    let mut msg = String::new();
    child.stderr.as_mut().unwrap().read_to_string(&mut msg);
    msg
}
