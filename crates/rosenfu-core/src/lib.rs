//! UIや保存方式に依存しない共通エンジンの入口。
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct EngineInfo {
    pub name: &'static str,
    pub version: &'static str,
}

pub fn engine_info() -> EngineInfo {
    EngineInfo {
        name: "Rosenfu",
        version: env!("CARGO_PKG_VERSION"),
    }
}

pub mod world;
