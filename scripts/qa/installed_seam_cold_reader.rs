use rosenfu_core::operation::TrainState;
use rosenfu_storage::{encode, load_world};

#[test]
fn cold_process_reopens_and_finishes_checkpoint() {
    for key in [
        "HOME",
        "USERPROFILE",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
        "XDG_STATE_HOME",
        "LOCALAPPDATA",
        "APPDATA",
    ] {
        let directory = std::env::var_os(key).unwrap();
        assert!(
            std::fs::read_dir(directory).unwrap().next().is_none(),
            "{key}"
        );
    }
    if std::env::var_os("ROSENFU_QA_SEAM_REQUIRE_NO_ROUTE").is_some() {
        assert!(
            std::fs::read_to_string("/proc/self/net/route")
                .unwrap()
                .lines()
                .count()
                <= 1
        );
        assert_eq!(
            std::fs::read_to_string("/proc/self/net/dev")
                .unwrap()
                .lines()
                .skip(2)
                .map(|line| line.trim().split_once(':').unwrap().0)
                .collect::<Vec<_>>(),
            ["lo"]
        );
    }
    let path = std::path::PathBuf::from(std::env::var_os("ROSENFU_QA_SEAM_CHECKPOINT").unwrap());
    let mut world = load_world(&path).unwrap();
    assert_eq!(encode(&world).unwrap(), std::fs::read(path).unwrap());
    let runtime = &mut world.operation_mut().unwrap().runtime;
    assert!(runtime
        .trains()
        .any(|t| t.key.service_id == "bus:A" && matches!(t.state, TrainState::Moving { .. })));
    assert!(runtime
        .trains()
        .any(|t| t.key.service_id == "bus:B" && matches!(t.state, TrainState::Waiting { .. })));
    let result = runtime.advance_to(120_000, 50).unwrap();
    assert_eq!(
        result
            .arrivals
            .iter()
            .map(|a| a.train.service_id.as_str())
            .collect::<Vec<_>>(),
        ["bus:A", "bus:B"]
    );
    assert!(result.arrivals[1].delay_ms > 0);
}
