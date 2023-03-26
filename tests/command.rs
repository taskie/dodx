use std::io::BufRead;

use itertools::Itertools;

macro_rules! test_filter {
    ($args: expr, $stdin: expr, $stdout: expr) => {
        test_filter!($args, $stdin, $stdout, "")
    };
    ($args: expr, $stdin: expr, $stdout: expr, $stderr: expr) => {
        let mut cmd = ::assert_cmd::Command::cargo_bin("dodx")?;
        let assert = cmd.args($args).write_stdin($stdin).assert();
        assert.success().stdout($stdout).stderr($stderr);
    };
}

// These test cases require `sed` command.

#[test]
fn test() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "s/e/E/g"],
        "tests/fixtures/example.txt",
        include_str!("fixtures/example.txt.patch")
    );
    Ok(())
}

#[test]
fn test_filter() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "-F", "s/e/E/g"],
        include_str!("fixtures/example.txt"),
        include_str!("fixtures/example.filter.patch")
    );
    Ok(())
}

#[test]
fn test_single_arg() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "-x", "s/e/E/g", "tests/fixtures/example.txt"],
        "",
        include_str!("fixtures/example.txt.patch")
    );
    Ok(())
}

#[test]
fn test_multi() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "s/e/E/g"],
        "tests/fixtures/example.txt\ntests/fixtures/example2.txt",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_multi_null() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "-0", "s/e/E/g"],
        "tests/fixtures/example.txt\0tests/fixtures/example2.txt",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_multi_args() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        [
            "sed",
            "-X",
            "s/e/E/g",
            "--",
            "tests/fixtures/example.txt",
            "tests/fixtures/example2.txt"
        ],
        "",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_multi_single_thread() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["-j", "1", "sed", "s/e/E/g"],
        "tests/fixtures/example.txt\ntests/fixtures/example2.txt",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_multi_unordered() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = ::assert_cmd::Command::cargo_bin("dodx")?;
    let assert = cmd
        .args(["-u", "sed", "s/e/E/g"])
        .write_stdin("tests/fixtures/example.txt\ntests/fixtures/example2.txt")
        .assert()
        .success()
        .stderr("");
    let output = assert.get_output();
    let expected_sort: Vec<_> = include_str!("fixtures/example.multi.patch")
        .lines()
        .sorted()
        .collect();
    let actual_sort: Vec<_> = output
        .stdout
        .lines()
        .filter_map(|l| l.ok())
        .sorted()
        .collect();
    assert_eq!(actual_sort, expected_sort);
    Ok(())
}

#[test]
fn test_multi_single_thread_unordered_force_parallel() -> Result<(), Box<dyn std::error::Error>> {
    // a hidden CLI option
    test_filter!(
        ["-j", "1", "-u", "--force-parallel", "sed", "s/e/E/g"],
        "tests/fixtures/example.txt\ntests/fixtures/example2.txt",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_files_from() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "-f", "tests/fixtures/example_files.txt", "s/e/E/g"],
        "",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_files_from_stdin() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "-f", "-", "s/e/E/g"],
        "tests/fixtures/example.txt\ntests/fixtures/example2.txt",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}

#[test]
fn test_files_from_stdin_null() -> Result<(), Box<dyn std::error::Error>> {
    test_filter!(
        ["sed", "-0f", "-", "s/e/E/g"],
        "tests/fixtures/example.txt\0tests/fixtures/example2.txt",
        include_str!("fixtures/example.multi.patch")
    );
    Ok(())
}
