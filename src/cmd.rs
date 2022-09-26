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
/// Those that can be measured by [time -l][time] & [gtime -v][gtime].
/// But several are unmaintained according to [getrusage(2)][getrusage]
/// Therefore, it is better to ignore data with all zeros.
///
/// [time]:https://www.freebsd.org/cgi/man.cgi?query=time
/// [gtime]:https://man7.org/linux/man-pages/man1/time.1.html
/// [getrusage]:https://man7.org/linux/man-pages/man2/getrusage.2.html
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum MeasItem {
    IGNORE,
    Real,
    User,
    Sys,
    CpuUsage,
    AvgSharedText,
    AvgUnsharedData,
    AvgStack,
    AvgTotal,
    MaxResident,
    AvgResident,
    MajorPageFault,
    MinorPageFault,
    VoluntaryCtxSwitch,
    InvoluntaryCtxSwitch,
    Swap,
    BlockInput,
    BlockOutput,
    MsgSend,
    MsgRecv,
    SignalRecv,
    Page,
    Instruction,
    Cycle,
    PeakMemory,
    Unknown(String),
}

pub fn meas_item_name(item: &MeasItem) -> &str {
    match item {
        MeasItem::IGNORE => "",
        MeasItem::Real => "Elapsed (wall clock) time",
        MeasItem::User => "User time",
        MeasItem::Sys => "System time",
        MeasItem::CpuUsage => "Percent of CPU this job got",
        MeasItem::AvgSharedText => "Average shared text size",
        MeasItem::AvgUnsharedData => "Average unshared data size",
        MeasItem::AvgStack => "Average stack size",
        MeasItem::AvgTotal => "Average total size",
        MeasItem::MaxResident => "Maximum resident set size",
        MeasItem::AvgResident => "Average resident set size",
        MeasItem::MajorPageFault => "Major (requiring I/O) page faults",
        MeasItem::MinorPageFault => "Minor (reclaiming a frame) page faults",
        MeasItem::VoluntaryCtxSwitch => "Voluntary context switches",
        MeasItem::InvoluntaryCtxSwitch => "Involuntary context switches",
        MeasItem::Swap => "Swaps",
        MeasItem::BlockInput => "Block by file system inputs",
        MeasItem::BlockOutput => "Block by file system outputs",
        MeasItem::MsgSend => "Socket messages sent",
        MeasItem::MsgRecv => "Socket messages received",
        MeasItem::SignalRecv => "Signals received",
        MeasItem::Page => "Page size",
        MeasItem::Instruction => "Instructions retired",
        MeasItem::Cycle => "Cycles elapsed",
        MeasItem::PeakMemory => "Peak memory footprint",
        MeasItem::Unknown(name) => name,
    }
}

pub fn meas_item_unit(item: &MeasItem) -> MeasUnit {
    match item {
        MeasItem::Real | MeasItem::User | MeasItem::Sys => MeasUnit::Second,
        MeasItem::CpuUsage => MeasUnit::Percent,
        MeasItem::MaxResident
        | MeasItem::AvgSharedText
        | MeasItem::AvgUnsharedData
        | MeasItem::AvgStack
        | MeasItem::AvgTotal
        | MeasItem::AvgResident
        | MeasItem::PeakMemory => MeasUnit::Byte,
        MeasItem::MajorPageFault
        | MeasItem::MinorPageFault
        | MeasItem::VoluntaryCtxSwitch
        | MeasItem::InvoluntaryCtxSwitch
        | MeasItem::Swap
        | MeasItem::BlockInput
        | MeasItem::BlockOutput
        | MeasItem::MsgSend
        | MeasItem::MsgRecv
        | MeasItem::SignalRecv
        | MeasItem::Instruction
        | MeasItem::Cycle
        | MeasItem::Page
        | MeasItem::Unknown(_)
        | MeasItem::IGNORE => MeasUnit::None,
    }
}

#[derive(Error, Debug)]
enum CmdError {
    #[error("Execution command is not ready yet. This is a bug in the source code.")]
    NotReady,
    #[error("Execution command is not finished yet. This is a bug in the source code.")]
    NotFinished,
    #[error("Could not parse the output of the `{0}` command. This is a source code issue, please provide the developer with the output of the `{0}` command.")]
    ParseError(&'static str),
}

#[derive(Debug)]
pub struct TimeCmd<'a> {
    sh: String,
    sh_arg: String,
    command: String,
    re: &'a regex::Regex,
    meas_item_map: std::collections::HashMap<&'a str, MeasItem>,
    process: std::process::Child,
    ready_status: ReadyStatus,
    meas_report: Option<std::collections::HashMap<MeasItem, f64>>,
}

pub fn try_new_builtin_time<'a>() -> anyhow::Result<TimeCmd<'a>> {
    TimeCmd::try_new_with_command(
        "bash",
        "-c",
        "time",
        builtin_re(),
        std::collections::HashMap::from([
            ("real", MeasItem::Real),
            ("user", MeasItem::User),
            ("sys", MeasItem::Sys),
        ]),
    )
}

fn builtin_re() -> &'static regex::Regex {
    static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?P<name>\w+)\s+(?:(?P<min>\d+)m)?(?P<sec>[0-9.]+)s?").unwrap()
    })
}

pub fn try_new_bsd_time<'a>() -> anyhow::Result<TimeCmd<'a>> {
    TimeCmd::try_new_with_command(
        "sh",
        "-c",
        "/usr/bin/env time -l",
        bsd_re(),
        std::collections::HashMap::from([
            ("real", MeasItem::Real),
            ("user", MeasItem::User),
            ("sys", MeasItem::Sys),
            ("maximum resident set size", MeasItem::MaxResident),
            ("average shared memory size", MeasItem::AvgSharedText),
            ("average unshared data size", MeasItem::AvgUnsharedData),
            ("average unshared stack size", MeasItem::AvgStack),
            ("page reclaims", MeasItem::MajorPageFault),
            ("page faults", MeasItem::MinorPageFault),
            ("swaps", MeasItem::Swap),
            ("block input operations", MeasItem::BlockInput),
            ("block output operations", MeasItem::BlockOutput),
            ("messages sent", MeasItem::MsgSend),
            ("messages received", MeasItem::MsgRecv),
            ("signals received", MeasItem::SignalRecv),
            ("voluntary context switches", MeasItem::VoluntaryCtxSwitch),
            (
                "involuntary context switches",
                MeasItem::InvoluntaryCtxSwitch,
            ),
            ("instructions retired", MeasItem::Instruction),
            ("cycles elapsed", MeasItem::Cycle),
            ("peak memory footprint", MeasItem::PeakMemory),
        ]),
    )
}

fn bsd_re() -> &'static regex::Regex {
    static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?m)(?P<val>[\d]+) +(?P<name>[\w ]+?)$|(?P<sec>[\d.]+) (?P<name_>\w+)")
            .unwrap()
    })
}

pub fn try_new_gnu_time<'a>(alias: bool) -> anyhow::Result<TimeCmd<'a>> {
    TimeCmd::try_new_with_command(
        "sh",
        "-c",
        if alias {
            "/usr/bin/env gtime -v"
        } else {
            "/usr/bin/env time -v"
        },
        gnu_re(),
        std::collections::HashMap::from([
            ("Command being timed", MeasItem::IGNORE),
            ("User time (seconds)", MeasItem::User),
            ("System time (seconds)", MeasItem::Sys),
            ("Percent of CPU this job got", MeasItem::CpuUsage),
            (
                "Elapsed (wall clock) time (h:mm:ss or m:ss)",
                MeasItem::Real,
            ),
            ("Average shared text size (kbytes)", MeasItem::AvgSharedText),
            (
                "Average unshared data size (kbytes)",
                MeasItem::AvgUnsharedData,
            ),
            ("Average stack size (kbytes)", MeasItem::AvgStack),
            ("Average total size (kbytes)", MeasItem::AvgTotal),
            ("Maximum resident set size (kbytes)", MeasItem::MaxResident),
            ("Average resident set size (kbytes)", MeasItem::AvgResident),
            (
                "Major (requiring I/O) page faults",
                MeasItem::MajorPageFault,
            ),
            (
                "Minor (reclaiming a frame) page faults",
                MeasItem::MinorPageFault,
            ),
            ("Voluntary context switches", MeasItem::VoluntaryCtxSwitch),
            (
                "Involuntary context switches",
                MeasItem::InvoluntaryCtxSwitch,
            ),
            ("Swaps", MeasItem::Swap),
            ("File system inputs", MeasItem::BlockInput),
            ("File system outputs", MeasItem::BlockOutput),
            ("Socket messages sent", MeasItem::MsgSend),
            ("Socket messages received", MeasItem::MsgRecv),
            ("Signals delivered", MeasItem::SignalRecv),
            ("Page size (bytes)", MeasItem::Page),
            ("Exit status", MeasItem::IGNORE),
        ]),
    )
}

fn gnu_re() -> &'static regex::Regex {
    static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?P<name>[\w ():/]+): ((?P<hour>\d+)??:?(?P<min>\d+):(?P<sec>[\d.]+)|(?P<val>[\d.]+))").unwrap()
    })
}
impl<'a> TimeCmd<'a> {
    pub fn try_new_with_command(
        sh: &str,
        sh_arg: &str,
        command: &str,
        re: &'a regex::Regex,
        meas_item_map: std::collections::HashMap<&'a str, MeasItem>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            sh: String::from(sh),
            sh_arg: String::from(sh_arg),
            command: String::from(command),
            re: re,
            meas_item_map: meas_item_map,
            // test to use
            process: execute(sh, &[sh_arg, format!("{} true", command).as_str()])?,
            ready_status: ReadyStatus::Checking,
            meas_report: None,
        })
    }

    pub fn ready_status(&mut self) -> ReadyStatus {
        if self.ready_status == ReadyStatus::Checking {
            if self.is_finished() {
                let err_msg = stderr(&mut self.process);
                if self.re.is_match(err_msg.as_str()) {
                    self.ready_status = ReadyStatus::Ready;
                } else {
                    self.ready_status = ReadyStatus::Error;
                }
            }
        }
        self.ready_status
    }

    pub fn execute(&mut self, command: &str) -> anyhow::Result<()> {
        anyhow::ensure!(self.ready_status == ReadyStatus::Ready, CmdError::NotReady);

        self.meas_report = None;
        self.process = execute(
            self.sh.as_str(),
            &[
                self.sh_arg.as_str(),
                format!("{} {}", self.command, command).as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn is_finished(&mut self) -> bool {
        self.process.try_wait().unwrap().is_some()
    }

    pub fn get_report(&mut self) -> anyhow::Result<&std::collections::HashMap<MeasItem, f64>> {
        anyhow::ensure!(self.is_finished(), CmdError::NotFinished);

        if self.meas_report.is_some() {
            return Ok(self.meas_report.as_ref().unwrap());
        }

        let mut meas_items = std::collections::HashMap::<MeasItem, f64>::new();
        let err_msg = stderr(&mut self.process);
        for cap in self.re.captures_iter(err_msg.as_str()) {
            let (name, v) = capture_name_and_val(&cap);
            if let Some(item) = self.meas_item_map.get(name) {
                if item != &MeasItem::IGNORE {
                    meas_items.insert(item.clone(), v);
                }
            } else {
                meas_items.insert(MeasItem::Unknown(String::from(name)), v);
            }
        }

        if meas_items.is_empty() {
            Err(CmdError::ParseError("time").into())
        } else {
            self.meas_report = Some(meas_items);
            Ok(self.meas_report.as_ref().unwrap())
        }
    }

    pub fn kill(&mut self) -> anyhow::Result<()> {
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

fn capture_name_and_val<'a>(cap: &'a regex::Captures) -> (&'a str, f64) {
    let v = if let Some(sec_match) = cap.name("sec") {
        let hour: f64 = if let Some(hour_match) = cap.name("hour") {
            hour_match.as_str().parse().unwrap()
        } else {
            0.0
        };
        let min: f64 = if let Some(min_match) = cap.name("min") {
            min_match.as_str().parse().unwrap()
        } else {
            0.0
        };
        let sec: f64 = sec_match.as_str().parse().unwrap();
        hour * 60.0 * 60.0 + min * 60.0 + sec
    } else {
        (&cap["val"]).parse().unwrap()
    };
    let name = if let Some(name_match) = cap.name("name") {
        name_match.as_str()
    } else {
        &cap["name_"]
    };
    (name, v)
}

#[cfg(test)]
mod test {
    use unindent::unindent;

    use super::*;

    #[test]
    fn builtin_re_match() {
        let output = unindent(
            "
            real	0m1.007s
            user	100m0.000s
            sys	0m0.001s
        ",
        );
        let expected =
            std::collections::HashMap::from([("real", 1.007), ("user", 6000.0), ("sys", 0.001)]);
        let mut actually = std::collections::HashMap::<String, f64>::new();
        for cap in builtin_re().captures_iter(output.as_str()) {
            let (name, v) = capture_name_and_val(&cap);
            actually.insert(String::from(name), v);
        }
        assert_eq!(expected.len(), actually.len());
        assert_eq!(
            expected.len(),
            actually
                .iter()
                .filter(|kvp| expected[kvp.0.as_str()] == *kvp.1)
                .count()
        );
    }

    #[test]
    fn bsd_re_match() {
        let output = unindent(
            "
            1.00 real         0.10 user         0.01 sys
                1277952  maximum resident set size
                    10  average shared memory size
                    11  average unshared data size
                    12  average unshared stack size
                    151  page reclaims
                    13  page faults
                    14  swaps
                    15  block input operations
                    16  block output operations
                    17  messages sent
                    18  messages received
                    19  signals received
                    20  voluntary context switches
                    3  involuntary context switches
                3056324  instructions retired
                1136018  cycles elapsed
                704896  peak memory footprint
        ",
        );
        let expected = std::collections::HashMap::from([
            ("real", 1.00),
            ("user", 0.10),
            ("sys", 0.01),
            ("maximum resident set size", 1277952.0),
            ("average shared memory size", 10.0),
            ("average unshared data size", 11.0),
            ("average unshared stack size", 12.0),
            ("page reclaims", 151.0),
            ("page faults", 13.0),
            ("swaps", 14.0),
            ("block input operations", 15.0),
            ("block output operations", 16.0),
            ("messages sent", 17.0),
            ("messages received", 18.0),
            ("signals received", 19.0),
            ("voluntary context switches", 20.0),
            ("involuntary context switches", 3.0),
            ("instructions retired", 3056324.0),
            ("cycles elapsed", 1136018.0),
            ("peak memory footprint", 704896.0),
        ]);
        let mut actually = std::collections::HashMap::<String, f64>::new();
        for cap in bsd_re().captures_iter(output.as_str()) {
            let (name, v) = capture_name_and_val(&cap);
            actually.insert(String::from(name), v);
        }
        assert_eq!(expected.len(), actually.len());
        assert_eq!(
            expected.len(),
            actually
                .iter()
                .filter(|kvp| expected[kvp.0.as_str()] == *kvp.1)
                .count()
        );
    }

    #[test]
    fn gnu_re_match() {
        let output = unindent(
            r#"
            Command being timed: "sleep 1"
            User time (seconds): 0.01
            System time (seconds): 0.02
            Percent of CPU this job got: 3%
            Elapsed (wall clock) time (h:mm:ss or m:ss): 10:01.00
            Average shared text size (kbytes): 4
            Average unshared data size (kbytes): 5
            Average stack size (kbytes): 7
            Average total size (kbytes): 8
            Maximum resident set size (kbytes): 1248
            Average resident set size (kbytes): 9
            Major (requiring I/O) page faults: 10
            Minor (reclaiming a frame) page faults: 152
            Voluntary context switches: 11
            Involuntary context switches: 6
            Swaps: 12
            File system inputs: 13
            File system outputs: 14
            Socket messages sent: 15
            Socket messages received: 16
            Signals delivered: 17
            Page size (bytes): 16384
            Exit status: 18
        "#,
        );
        let expected = std::collections::HashMap::from([
            ("User time (seconds)", 0.01),
            ("System time (seconds)", 0.02),
            ("Percent of CPU this job got", 3.0),
            ("Elapsed (wall clock) time (h:mm:ss or m:ss)", 601.0),
            ("Average shared text size (kbytes)", 4.0),
            ("Average unshared data size (kbytes)", 5.0),
            ("Average stack size (kbytes)", 7.0),
            ("Average total size (kbytes)", 8.0),
            ("Maximum resident set size (kbytes)", 1248.0),
            ("Average resident set size (kbytes)", 9.0),
            ("Major (requiring I/O) page faults", 10.0),
            ("Minor (reclaiming a frame) page faults", 152.0),
            ("Voluntary context switches", 11.0),
            ("Involuntary context switches", 6.0),
            ("Swaps", 12.0),
            ("File system inputs", 13.0),
            ("File system outputs", 14.0),
            ("Socket messages sent", 15.0),
            ("Socket messages received", 16.0),
            ("Signals delivered", 17.0),
            ("Page size (bytes)", 16384.0),
            ("Exit status", 18.0),
        ]);
        let mut actually = std::collections::HashMap::<String, f64>::new();
        for cap in gnu_re().captures_iter(output.as_str()) {
            let (name, v) = capture_name_and_val(&cap);
            actually.insert(String::from(name), v);
        }
        assert_eq!(expected.len(), actually.len());
        assert_eq!(
            expected.len(),
            actually
                .iter()
                .filter(|kvp| expected[kvp.0.as_str()] == *kvp.1)
                .count()
        );
    }
}
