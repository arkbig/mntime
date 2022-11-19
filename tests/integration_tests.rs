mod common;
use common::mntime;

use predicates::prelude::PredicateBooleanExt as _;

#[test]
fn runs_successfully() {
    mntime()
        .arg("--runs=2")
        .arg("echo dummy benchmark")
        .assert()
        .success();
}

#[test]
fn one_run_is_supported() {
    mntime()
        .arg("--runs=1")
        .arg("echo dummy benchmark")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Benchmark #1> echo dummy benchmark",
        ));
}

#[test]
fn two_run_is_supported() {
    mntime()
        .arg("--runs=1")
        .arg("echo dummy benchmark")
        .arg("echo dummy benchmark 2")
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Benchmark #1> echo dummy benchmark").and(
                predicates::str::contains("Benchmark #2> echo dummy benchmark 2"),
            ),
        );
}

#[test]
fn many_run_is_supported() {
    mntime()
        .arg("--runs=1")
        .args(["echo", "dummy", "benchmark"])
        .arg("--")
        .args(["echo", "dummy", "benchmark", "2"])
        .arg("--")
        .args(["echo", "dummy", "benchmark", "3"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Benchmark #1> echo 'dummy' 'benchmark'").and(
                predicates::str::contains("Benchmark #2> echo 'dummy' 'benchmark' '2'").and(
                    predicates::str::contains("Benchmark #3> echo 'dummy' 'benchmark' '3'"),
                ),
            ),
        );
}

#[test]
fn failure_command_is_supported() {
    mntime()
        .arg("--runs=3")
        .arg("false")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Exit status: Success 0 times. Failure 3 times. [(code× times)",
        ));
}

#[test]
fn run_count_change_is_supported() {
    mntime()
        .arg("--runs=3")
        .arg("echo dummy benchmark")
        .assert()
        .success()
        .stdout(predicates::str::contains("/ 3\r\n"));
}

#[test]
fn execution_count_change_is_supported() {
    let cmd = std::process::Command::new("bash")
        .args(&["-c", "echo ok"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let o = cmd.unwrap().wait_with_output().unwrap();
    let f = format!(
        "{}\r\n[OUT]{}\r\n[ERR]{}",
        o.status,
        std::str::from_utf8(&o.stdout).unwrap(),
        std::str::from_utf8(&o.stderr).unwrap()
    );
    println!("{}", f);
    assert!(f.contains("ok"));

    mntime()
        .arg("--runs=2")
        .arg("--loops=3")
        .arg("echo dummy benchmark")
        .assert()
        .success()
        .stdout(predicates::str::contains("/3 "));
}

#[test]
fn shell_change_is_supported() {
    let cmd = std::process::Command::new("bash")
        .args(&["-c", "time 'sleep' 1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let o = cmd.unwrap().wait_with_output().unwrap();
    let f = format!(
        "{}\r\n[OUT]{}\r\n[ERR]{}",
        o.status,
        std::str::from_utf8(&o.stdout).unwrap(),
        std::str::from_utf8(&o.stderr).unwrap()
    );
    println!("{}", f);
    assert!(f.contains("ok"));

    mntime()
        .arg("--runs=1")
        .arg("--shell=bash")
        .arg("echo dummy benchmark")
        .assert()
        .success();
}

#[test]
fn only_using_builtin_time_is_supported() {
    mntime()
        .arg("--runs=1")
        .arg("--use-builtin-only")
        .arg("echo dummy benchmark")
        .assert()
        .success()
        .stdout(predicates::str::contains("Reclaiming a frame page faults:").not());
}

#[test]
fn warns_about_missing_bsd_time_commands() {
    mntime()
        .arg("--runs=1")
        .arg("--bsd=/this_will_never_exist")
        .arg("--no-gnu")
        .arg("echo dummy benchmark")
        .assert()
        .success()
        .stdout(predicates::str::contains("Percent of CPU this job got").not())
        .stderr(predicates::str::contains(
            "[WARNING]: The bsd time command not found.",
        ));
}

#[test]
fn warns_about_missing_gnu_time_commands() {
    mntime()
        .arg("--runs=1")
        .arg("--no-bsd")
        .arg("--gnu=/this_will_never_exist")
        .arg("echo dummy benchmark")
        .assert()
        .success()
        .stdout(predicates::str::contains("Instructions retired").not())
        .stderr(predicates::str::contains(
            "[WARNING]: The gnu time command not found.",
        ));
}