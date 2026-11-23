use super::WorldEditor;
use rosenfu_core::world::PlannedCrossingDelayError;

const INSPECTOR_HOUR_MS: u64 = 3_600_000;
const MAX_VISIBLE: usize = 25;

#[derive(serde::Serialize)]
pub struct CrossingPlanningView {
    configuration_token: String,
    start_ms: String,
    end_ms: String,
    available: bool,
    estimate_count: usize,
    estimates: Vec<CrossingEstimate>,
    incomplete_count: usize,
    incomplete_crossings: Vec<[String; 2]>,
    sparse_count: usize,
    sparse_crossings: Vec<[String; 2]>,
}

#[derive(serde::Serialize)]
struct CrossingEstimate {
    alignment_ids: [String; 2],
    approach_alignment_id: String,
    towards_alignment_end: bool,
    scheduled_passages: [u64; 2],
    requested_utilization: f64,
    estimated_delay_seconds_per_train: f64,
}

impl WorldEditor {
    /// Clock motion alone never invalidates this no-conflict timetable view.
    /// Check the world and timetable revision under the same lock as the read.
    pub fn preview_crossing_planning(
        &self,
        expected_configuration_token: &str,
        start_ms: &str,
    ) -> Result<CrossingPlanningView, String> {
        let configuration_token = self.operation_configuration_token();
        if configuration_token != expected_configuration_token {
            return Err(
                "ワールドまたは運行設定が変更されました。最新の状態で調べ直してください。".into(),
            );
        }
        if start_ms.len() > 20 {
            return Err("集計開始時刻を確認してください。".into());
        }
        let start = start_ms
            .parse::<u64>()
            .ok()
            .filter(|v| v.to_string() == start_ms)
            .ok_or("集計開始時刻を確認してください。")?;
        let end = start
            .checked_add(INSPECTOR_HOUR_MS)
            .ok_or("集計時間が時刻の上限を超えます。")?;
        let available = self
            .world
            .operation()
            .is_some_and(|o| !o.invalidated && !o.configs.is_empty());
        let mut preview = if available {
            self.world
                .preview_planned_crossing_delay(start)
                .map_err(|err| match err {
                    #[cfg(feature = "standard-periodic-rail")]
                    PlannedCrossingDelayError::StandardTimetableUnsupported => {
                        "時間帯ダイヤの平面交差試算は未対応です。"
                    }
                    PlannedCrossingDelayError::WindowOverflow => "集計時間が時刻の上限を超えます。",
                    PlannedCrossingDelayError::WorkBudgetExceeded => {
                        "運行設定が大きすぎるため、この計画試算はできません。"
                    }
                })?
        } else {
            Default::default()
        };
        preview.estimates.sort_by(|a, b| {
            b.estimated_delay_seconds_per_train
                .total_cmp(&a.estimated_delay_seconds_per_train)
        });
        let ids = |pair: [u64; 2]| pair.map(|id| id.to_string());
        Ok(CrossingPlanningView {
            configuration_token,
            start_ms: start.to_string(),
            end_ms: end.to_string(),
            available,
            estimate_count: preview.estimates.len(),
            estimates: preview
                .estimates
                .into_iter()
                .take(MAX_VISIBLE)
                .map(|r| CrossingEstimate {
                    alignment_ids: ids(r.alignment_ids),
                    approach_alignment_id: r.approach_alignment_id.to_string(),
                    towards_alignment_end: r.towards_alignment_end,
                    scheduled_passages: r.scheduled_passages,
                    requested_utilization: r.requested_utilization,
                    estimated_delay_seconds_per_train: r.estimated_delay_seconds_per_train,
                })
                .collect(),
            incomplete_count: preview.incomplete_crossings.len(),
            incomplete_crossings: preview
                .incomplete_crossings
                .into_iter()
                .take(MAX_VISIBLE)
                .map(ids)
                .collect(),
            sparse_count: preview.sparse_crossings.len(),
            sparse_crossings: preview
                .sparse_crossings
                .into_iter()
                .take(MAX_VISIBLE)
                .map(ids)
                .collect(),
        })
    }
}
