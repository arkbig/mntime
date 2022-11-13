use assert_cmd::cargo::CommandCargoExt as _;

pub fn mntime_raw_command() -> std::process::Command {
    let mut cmd = std::process::Command::cargo_bin("mntime").unwrap();
    cmd.current_dir("tests/");
    cmd
}

pub fn mntime() -> assert_cmd::Command {
    assert_cmd::Command::from_std(mntime_raw_command())
}
