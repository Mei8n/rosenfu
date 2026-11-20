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

/// Bounded, offline OpenFreeMap tile evidence decoding (not yet world storage).
pub mod ofm_evidence;
pub mod water_observations;

/// Independent offline reader; not wired to a provider or construction session.
pub mod water_zone;

pub mod demand;
/// Shared clock, timetable and headless resource-constrained operation.
pub mod operation;
