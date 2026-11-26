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

/// Fixed synthetic OD reference demand and initial passenger choice.
pub mod demand;
/// Shared clock, timetable and headless resource-constrained operation.
pub mod operation;
/// Explicit road-transport domain; display evidence alone never grants access.
pub mod road_transport;
