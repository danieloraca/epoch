use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_epoch").to_string()
}

#[test]
fn cli_default_outputs_single_line() {
    let out = Command::new(bin())
        .arg("1700000000")
        .output()
        .expect("run timeparse");

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    // single line + newline
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.starts_with("2023-11-14T"));
}

#[test]
fn cli_unix_outputs_integer() {
    let out = Command::new(bin())
        .arg("1700000000")
        .arg("--unix")
        .output()
        .expect("run timeparse");

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "1700000000");
}

#[test]
fn cli_json_is_valid_json_pretty() {
    let out = Command::new(bin())
        .arg("1700000000")
        .arg("--json")
        .output()
        .expect("run timeparse");

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();

    // pretty json typically starts with "{\n"
    assert!(stdout.starts_with("{\n"));

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["unix_seconds"], 1700000000);
    assert!(v["rfc3339"].as_str().unwrap().starts_with("2023-11-14T"));
}
