fn main() {
    println!(
        "{}",
        serde_json::to_string(&rosenfu_core::engine_info()).expect("engine info is serializable")
    );
}
