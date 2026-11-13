use std::process::Command;

#[test]
fn headless_executable_returns_shared_engine_info() {
    let output = Command::new(env!("CARGO_BIN_EXE_rosenfu-cli"))
        .output()
        .expect("CLI starts without a desktop");
    assert!(output.status.success());
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = serde_json::to_value(rosenfu_core::engine_info()).unwrap();
    assert_eq!(actual, expected);
    assert!(output.stderr.is_empty());
}
