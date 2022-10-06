use anyhow::Context;
use num_format::ToFormattedString;
use std::io::Read;
use strum::{EnumIter, IntoEnumIterator};
use thiserror::Error;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ReadyStatus {
    Checking,
    Ready,
    Error,
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
#[derive(Debug, Hash, Eq, PartialEq, Clone, EnumIter)]
pub enum MeasItem {
    ExitStatus,
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
        MeasItem::ExitStatus => "Exit status",
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
        MeasItem::MajorPageFault => "Requiring I/O page faults",
        MeasItem::MinorPageFault => "Reclaiming a frame page faults",
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

pub fn meas_item_name_max_width() -> usize {
    static WIDTH: once_cell::sync::OnceCell<usize> = once_cell::sync::OnceCell::new();
    *WIDTH.get_or_init(|| {
        let mut width = 0;
        for item in MeasItem::iter() {
            width = std::cmp::max(width, meas_item_name(&item).len());
        }
        width
    })
}

pub fn meas_item_unit_value(item: &MeasItem, val: f64) -> String {
    match item {
        MeasItem::Real | MeasItem::User | MeasItem::Sys => {
            if val < 1.0 {
                format!("{} ms", round_precision(val * 1000.0, 3))
            } else if val < 60.0 {
                format!("{} sec", round_precision(val, 3))
            } else if val < 60.0 * 60.0 {
                let min = (val / 60.0).floor();
                format!("{:02}:{} sec", min, round_precision(val % 60.0, 3))
            } else {
                let hour = (val / 60.0 / 60.0).floor();
                let min = ((val - hour * 60.0 * 60.0) / 60.0).floor();
                format!(
                    "{:02}:{:02}:{} sec",
                    hour,
                    min,
                    round_precision(val % 60.0, 3)
                )
            }
        }
        MeasItem::CpuUsage => {
            format!("{} %", val)
        }
        MeasItem::MaxResident
        | MeasItem::AvgSharedText
        | MeasItem::AvgUnsharedData
        | MeasItem::AvgStack
        | MeasItem::AvgTotal
        | MeasItem::AvgResident
        | MeasItem::PeakMemory => {
            const KB: f64 = 1024.0;
            const MB: f64 = 1024.0 * KB;
            const GB: f64 = 1024.0 * MB;
            const TB: f64 = 1024.0 * GB;
            if val < KB {
                format!("{} byte", round_precision(val, 3))
            } else if val < MB {
                format!("{} KiB", round_precision(val / KB, 3))
            } else if val < GB {
                format!("{} MiB", round_precision(val / MB, 3))
            } else if val < TB {
                format!("{} GiB", round_precision(val / GB, 3))
            } else {
                format!("{} TiB", round_precision(val / TB, 3))
            }
        }
        MeasItem::ExitStatus
        | MeasItem::MajorPageFault
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
        | MeasItem::Unknown(_) => {
            let int = val.floor() as i64;
            let dec = format!("{}", round_precision(val - int as f64, 3));
            if dec == "0" {
                int.to_formatted_string(&num_format::Locale::en)
            } else {
                int.to_formatted_string(&num_format::Locale::en) + &dec[1..]
            }
        }
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

pub struct TimeCmd {
    sh: String,
    sh_arg: String,
    command: String,
    process: std::process::Child,
    ready_status: ReadyStatus,
    parse_meas_items: fn(&str) -> std::collections::HashMap<crate::cmd::MeasItem, f64>,
    meas_report: Option<std::collections::HashMap<MeasItem, f64>>,
}

pub fn try_new_builtin_time() -> anyhow::Result<TimeCmd> {
    TimeCmd::try_new_with_command("bash", "-c", "time", |err_msg| {
        let mut meas_items = std::collections::HashMap::<MeasItem, f64>::new();
        let re = builtin_re();
        for cap in re.captures_iter(err_msg) {
            let (name, v) = capture_name_and_val(&cap);
            match name {
                "real" => meas_items.insert(MeasItem::Real, v),
                "user" => meas_items.insert(MeasItem::User, v),
                "sys" => meas_items.insert(MeasItem::Sys, v),
                _ => meas_items.insert(MeasItem::Unknown(String::from(name)), v),
            };
        }
        meas_items
    })
}

fn builtin_re() -> &'static regex::Regex {
    static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?P<name>\w+)\s+(?:(?P<min>\d+)m)?(?P<sec>[0-9.]+)s?").unwrap()
    })
}

pub fn try_new_bsd_time() -> anyhow::Result<TimeCmd> {
    TimeCmd::try_new_with_command("sh", "-c", "/usr/bin/env time -l", |err_msg| {
        let mut meas_items = std::collections::HashMap::<MeasItem, f64>::new();
        let re = bsd_re();
        for cap in re.captures_iter(err_msg) {
            let (name, v) = capture_name_and_val(&cap);
            match name {
                "real" => meas_items.insert(MeasItem::Real, v),
                "user" => meas_items.insert(MeasItem::User, v),
                "sys" => meas_items.insert(MeasItem::Sys, v),
                "maximum resident set size" => meas_items.insert(MeasItem::MaxResident, v),
                "average shared memory size" => meas_items.insert(MeasItem::AvgSharedText, v),
                "average unshared data size" => meas_items.insert(MeasItem::AvgUnsharedData, v),
                "average unshared stack size" => meas_items.insert(MeasItem::AvgStack, v),
                "page reclaims" => meas_items.insert(MeasItem::MinorPageFault, v),
                "page faults" => meas_items.insert(MeasItem::MajorPageFault, v),
                "swaps" => meas_items.insert(MeasItem::Swap, v),
                "block input operations" => meas_items.insert(MeasItem::BlockInput, v),
                "block output operations" => meas_items.insert(MeasItem::BlockOutput, v),
                "messages sent" => meas_items.insert(MeasItem::MsgSend, v),
                "messages received" => meas_items.insert(MeasItem::MsgRecv, v),
                "signals received" => meas_items.insert(MeasItem::SignalRecv, v),
                "voluntary context switches" => meas_items.insert(MeasItem::VoluntaryCtxSwitch, v),
                "involuntary context switches" => {
                    meas_items.insert(MeasItem::InvoluntaryCtxSwitch, v)
                }
                "instructions retired" => meas_items.insert(MeasItem::Instruction, v),
                "cycles elapsed" => meas_items.insert(MeasItem::Cycle, v),
                "peak memory footprint" => meas_items.insert(MeasItem::PeakMemory, v),
                _ => meas_items.insert(MeasItem::Unknown(String::from(name)), v),
            };
        }
        meas_items
    })
}

fn bsd_re() -> &'static regex::Regex {
    static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?m)(?P<val>[\d]+) +(?P<name>[\w ]+?)$|(?P<sec>[\d.]+) (?P<name_>\w+)")
            .unwrap()
    })
}

pub fn try_new_gnu_time(alias: bool) -> anyhow::Result<TimeCmd> {
    TimeCmd::try_new_with_command(
        "sh",
        "-c",
        if alias {
            "/usr/bin/env gtime -v"
        } else {
            "/usr/bin/env time -v"
        },
        |err_msg| {
            let mut meas_items = std::collections::HashMap::<MeasItem, f64>::new();
            let re = gnu_re();
            const KB: f64 = 1024.0;
            for cap in re.captures_iter(err_msg) {
                let (name, v) = capture_name_and_val(&cap);
                match name {
                    "Command being timed" => {}
                    "User time (seconds)" => {
                        meas_items.insert(MeasItem::User, v);
                    }
                    "System time (seconds)" => {
                        meas_items.insert(MeasItem::Sys, v);
                    }
                    "Percent of CPU this job got" => {
                        meas_items.insert(MeasItem::CpuUsage, v);
                    }
                    "Elapsed (wall clock) time (h:mm:ss or m:ss)" => {
                        meas_items.insert(MeasItem::Real, v);
                    }
                    "Average shared text size (kbytes)" => {
                        meas_items.insert(MeasItem::AvgSharedText, v * KB);
                    }
                    "Average unshared data size (kbytes)" => {
                        meas_items.insert(MeasItem::AvgUnsharedData, v * KB);
                    }
                    "Average stack size (kbytes)" => {
                        meas_items.insert(MeasItem::AvgStack, v * KB);
                    }
                    "Average total size (kbytes)" => {
                        meas_items.insert(MeasItem::AvgTotal, v * KB);
                    }
                    "Maximum resident set size (kbytes)" => {
                        meas_items.insert(MeasItem::MaxResident, v * KB);
                    }
                    "Average resident set size (kbytes)" => {
                        meas_items.insert(MeasItem::AvgResident, v * KB);
                    }
                    "Major (requiring I/O) page faults" => {
                        meas_items.insert(MeasItem::MajorPageFault, v);
                    }
                    "Minor (reclaiming a frame) page faults" => {
                        meas_items.insert(MeasItem::MinorPageFault, v);
                    }
                    "Voluntary context switches" => {
                        meas_items.insert(MeasItem::VoluntaryCtxSwitch, v);
                    }
                    "Involuntary context switches" => {
                        meas_items.insert(MeasItem::InvoluntaryCtxSwitch, v);
                    }
                    "Swaps" => {
                        meas_items.insert(MeasItem::Swap, v);
                    }
                    "File system inputs" => {
                        meas_items.insert(MeasItem::BlockInput, v);
                    }
                    "File system outputs" => {
                        meas_items.insert(MeasItem::BlockOutput, v);
                    }
                    "Socket messages sent" => {
                        meas_items.insert(MeasItem::MsgSend, v);
                    }
                    "Socket messages received" => {
                        meas_items.insert(MeasItem::MsgRecv, v);
                    }
                    "Signals delivered" => {
                        meas_items.insert(MeasItem::SignalRecv, v);
                    }
                    "Page size (bytes)" => {
                        meas_items.insert(MeasItem::Page, v);
                    }
                    "Exit status" => {
                        meas_items.insert(MeasItem::ExitStatus, v);
                    }
                    _ => {
                        meas_items.insert(MeasItem::Unknown(String::from(name)), v);
                    }
                };
            }
            meas_items
        },
    )
}

fn gnu_re() -> &'static regex::Regex {
    static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?P<name>[\w ():/]+): ((?P<hour>\d+)??:?(?P<min>\d+):(?P<sec>[\d.]+)|(?P<val>[\d.]+))").unwrap()   
    })
}

impl TimeCmd {
    pub fn try_new_with_command(
        sh: &str,
        sh_arg: &str,
        command: &str,
        parse_meas_items: fn(&str) -> std::collections::HashMap<crate::cmd::MeasItem, f64>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            sh: String::from(sh),
            sh_arg: String::from(sh_arg),
            command: String::from(command),
            parse_meas_items,
            // test to use
            process: execute(sh, &[sh_arg, format!("{} true", command).as_str()])?,
            ready_status: ReadyStatus::Checking,
            meas_report: None,
        })
    }

    pub fn ready_status(&mut self) -> ReadyStatus {
        if self.ready_status == ReadyStatus::Checking && self.is_finished() {
            let err_msg = stderr(&mut self.process);
            if (self.parse_meas_items)(err_msg.as_str()).is_empty() {
                self.ready_status = ReadyStatus::Error;
            } else {
                self.ready_status = ReadyStatus::Ready;
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

        let err_msg = stderr(&mut self.process);
        let mut meas_items = (self.parse_meas_items)(err_msg.as_str());
        if meas_items.is_empty() {
            Err(CmdError::ParseError("time").into())
        } else {
            if meas_items.get(&MeasItem::ExitStatus).is_none() {
                meas_items.insert(
                    MeasItem::ExitStatus,
                    self.process.wait().unwrap().code().unwrap_or_default() as f64,
                );
            }
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
    child
        .stderr
        .as_mut()
        .unwrap()
        .read_to_string(&mut msg)
        .unwrap();
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

fn round_precision(val: f64, precision: i32) -> f64 {
    let rank = 10f64.powi(precision);
    (val * rank).round() / rank
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn builtin_re_match() {
        let output = r#"
            real	0m1.007s
            user	100m0.000s
            sys	0m0.001s
        "#;
        let expected =
            std::collections::HashMap::from([("real", 1.007), ("user", 6000.0), ("sys", 0.001)]);
        let mut actually = std::collections::HashMap::<String, f64>::new();
        for cap in builtin_re().captures_iter(output) {
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
        let output = r#"
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
        "#;
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
        for cap in bsd_re().captures_iter(output) {
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
        let output = r#"
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
        "#;
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
        for cap in gnu_re().captures_iter(output) {
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
    fn meas_item_unit_value_sec() {
        assert_eq!(
            "123.457 ms",
            meas_item_unit_value(&MeasItem::Real, 0.123456789)
        );
        assert_eq!(
            "12.346 sec",
            meas_item_unit_value(&MeasItem::Real, 12.3456789)
        );
        assert_eq!(
            "01:23.457 sec",
            meas_item_unit_value(&MeasItem::Real, 60.0 + 23.456789)
        );
        assert_eq!(
            "59:23.457 sec",
            meas_item_unit_value(&MeasItem::Real, 59.0 * 60.0 + 23.456789)
        );
        assert_eq!(
            "123:04:56.789 sec",
            meas_item_unit_value(&MeasItem::Real, 123.0 * 60.0 * 60.0 + 4.0 * 60.0 + 56.789)
        );
    }

    #[test]
    fn meas_item_unit_value_byte() {
        assert_eq!(
            "123.457 byte",
            meas_item_unit_value(&MeasItem::MaxResident, 123.456789)
        );
        assert_eq!(
            "12.346 KiB",
            meas_item_unit_value(&MeasItem::MaxResident, 12.3456789 * 1024.0)
        );
        assert_eq!(
            "123.457 MiB",
            meas_item_unit_value(&MeasItem::MaxResident, 123.456789 * 1024.0 * 1024.0)
        );
        assert_eq!(
            "123.457 GiB",
            meas_item_unit_value(
                &MeasItem::MaxResident,
                123.456789 * 1024.0 * 1024.0 * 1024.0
            )
        );
        assert_eq!(
            "1234.568 TiB",
            meas_item_unit_value(
                &MeasItem::MaxResident,
                1234.56789 * 1024.0 * 1024.0 * 1024.0 * 1024.0
            )
        );
    }

    #[test]
    fn meas_item_unit_value_digit() {
        assert_eq!(
            "123.457",
            meas_item_unit_value(&MeasItem::Cycle, 123.456789)
        );
        assert_eq!(
            "123,456.789",
            meas_item_unit_value(&MeasItem::Cycle, 123_456.789)
        );
        assert_eq!(
            "123,456,789",
            meas_item_unit_value(&MeasItem::Cycle, 123456789.0)
        );
        assert_eq!(
            "123,456,789,012.346",
            meas_item_unit_value(&MeasItem::Cycle, 123_456_789_012.3456789)
        );
        assert_eq!(
            "123,456,789,012,345",
            meas_item_unit_value(&MeasItem::Cycle, 123_456_789_012_345.0)
        );
        assert_eq!(
            "123,456,789,012,345.672",
            meas_item_unit_value(&MeasItem::Cycle, 123_456_789_012_345.6789)
        );
    }
}
