//! Height redevelopment confined to an explicitly selected continuous run.
use super::*;
impl World {
    fn split_at_distance(
        &mut self,
        owner: CompanyId,
        id: AlignmentId,
        distance_m: f64,
    ) -> Result<AlignmentId, WorldError> {
        let line = self
            .data
            .alignments
            .iter()
            .find(|a| a.id == id)
            .ok_or(WorldError::InvalidAlignment)?;
        let mut remaining = distance_m;
        for (segment, pair) in line.points.windows(2).enumerate() {
            let length = super::alignment::distance(pair[0], pair[1]);
            if remaining <= length + 1e-7 {
                let fraction = if (remaining - length).abs() < 1e-7 {
                    1.0
                } else if remaining.abs() < 1e-7 {
                    0.0
                } else {
                    (remaining / length).clamp(0., 1.)
                };
                return self.split_alignment_at_position(owner, id, segment, fraction);
            }
            remaining -= length;
        }
        Err(WorldError::InvalidAlignment)
    }
    /// Return the three replacement spans, keeping all work inside selection.
    /// Endpoint heights/grades and outside platform locations are immutable.
    pub fn redevelop_alignment_section_height(
        &mut self,
        owner: CompanyId,
        id: AlignmentId,
        first: usize,
        last: usize,
        level: InfrastructureLevel,
    ) -> Result<[AlignmentId; 3], WorldError> {
        let original = self
            .data
            .alignments
            .iter()
            .find(|a| a.id == id && a.owner == owner)
            .ok_or(WorldError::MissingReference("height.alignment"))?
            .clone();
        let anchors = original
            .anchor_indices
            .clone()
            .unwrap_or_else(|| vec![0, original.points.len() - 1]);
        if first > last || last + 1 >= anchors.len() {
            return Err(WorldError::InvalidAlignment);
        }
        let prefix_length = |end: usize| {
            original.points[..=end]
                .windows(2)
                .map(|p| super::alignment::distance(p[0], p[1]))
                .sum::<f64>()
        };
        let total = original.length_m();
        let lo = prefix_length(anchors[first]);
        let hi = prefix_length(anchors[last + 1]);
        let length = hi - lo;
        // Moving an existing platform vertically needs an explicit station edit.
        if self.data.station_placements.iter().any(|p| {
            p.alignment_id == id
                && p.position * total + p.length_m / 2.0 > lo
                && p.position * total - p.length_m / 2.0 < hi
        }) {
            return Err(WorldError::HeightRedevelopmentStation);
        }
        let h0 = original
            .height_at_position_m(lo / total)
            .ok_or(WorldError::InvalidAlignment)?;
        let h1 = original
            .height_at_position_m(hi / total)
            .ok_or(WorldError::InvalidAlignment)?;
        let g0 = original
            .height_and_grade_at_position(lo / total)
            .ok_or(WorldError::InvalidAlignment)?
            .1;
        let g1 = original
            .height_and_grade_at_position(hi / total)
            .ok_or(WorldError::InvalidAlignment)?
            .1;
        let target = level.height_m();
        let required = |from: f64, to: f64, start_grade: f64, end_grade: f64| -> Option<f64> {
            let valid = |l: f64| {
                VerticalProfile {
                    start_height_m: from,
                    end_height_m: to,
                    start_tangent_m: start_grade * l,
                    end_tangent_m: end_grade * l,
                }
                .is_valid(l, original.track_type)
            };
            let mut upper = 20.;
            while !valid(upper) && upper < length {
                upper *= 2.;
            }
            if !valid(upper) {
                return None;
            }
            let mut lower = 20.;
            for _ in 0..48 {
                let mid = (lower + upper) / 2.;
                if valid(mid) {
                    upper = mid;
                } else {
                    lower = mid;
                }
            }
            Some(upper * (1. + 1e-8))
        };
        let entry_length =
            required(h0, target, g0, 0.).ok_or(WorldError::HeightRedevelopmentSpace)?;
        let exit_length =
            required(target, h1, 0., g1).ok_or(WorldError::HeightRedevelopmentSpace)?;
        if entry_length + exit_length + 20. > length {
            return Err(WorldError::HeightRedevelopmentSpace);
        }
        let mut candidate = self.clone();
        if hi < total - 1e-8 {
            candidate.split_at_distance(owner, id, hi)?;
        }
        let selected = if lo > 1e-8 {
            candidate.split_at_distance(owner, id, lo)?
        } else {
            id
        };
        let exit = candidate.split_at_distance(owner, selected, length - exit_length)?;
        let plateau = candidate.split_at_distance(owner, selected, entry_length)?;
        let mut draft = candidate.data.clone();
        for (span, start, end, start_grade, end_grade) in [
            (selected, h0, target, g0, 0.),
            (plateau, target, target, 0., 0.),
            (exit, target, h1, 0., g1),
        ] {
            let line = draft.alignments.iter_mut().find(|a| a.id == span).unwrap();
            let len = line.length_m();
            let mut infrastructure = line.effective_infrastructure();
            for section in &mut infrastructure {
                section.level = level;
            }
            line.infrastructure = infrastructure;
            line.section_profiles.clear();
            line.vertical_profile = Some(VerticalProfile {
                start_height_m: start,
                end_height_m: end,
                start_tangent_m: start_grade * len,
                end_tangent_m: end_grade * len,
            });
        }
        candidate.replace(draft)?;
        *self = candidate;
        Ok([selected, plateau, exit])
    }
}
