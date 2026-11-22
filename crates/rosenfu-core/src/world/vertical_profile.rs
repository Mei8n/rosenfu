//! Construction-only vertical geometry. It never alters vehicle speed.
use super::*;

/// Owner-selected spacing on the sandbox flat reference plane.
/// Persisted profiles store metres, independently of future layer spacing.
pub const LEVEL_SPACING_M: f64 = 6.0;

impl InfrastructureLevel {
    pub fn height_m(self) -> f64 {
        let index = match self {
            Self::Ground => 0,
            Self::Elevated => 1,
            Self::Elevated2 => 2,
            Self::Elevated3 => 3,
            Self::Underground => -1,
            Self::Underground2 => -2,
            Self::Underground3 => -3,
        };
        f64::from(index) * LEVEL_SPACING_M
    }
}
impl TrackType {
    /// Owner-selected construction limits. No speed simulation correction.
    pub fn maximum_construction_gradient_per_mille(self) -> f64 {
        match self {
            Self::Normal | Self::Tram => 45.0,
            Self::HighSpeed => 30.0,
        }
    }
}

/// Cubic Hermite height over normalized horizontal arc length.
/// Zero endpoint tangents give smoothstep; clipped profiles retain nonzero
/// tangents. Maximum grade is checked at endpoints and derivative extrema.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerticalProfile {
    pub start_height_m: f64,
    /// Metres per normalized arc-length parameter; zero gives a flat endpoint.
    #[serde(default)]
    pub start_tangent_m: f64,
    #[serde(default)]
    pub end_tangent_m: f64,
    pub end_height_m: f64,
}
impl VerticalProfile {
    pub fn height_at(self, t: f64) -> f64 {
        let t2 = t * t;
        let t3 = t2 * t;
        (2.0 * t3 - 3.0 * t2 + 1.0) * self.start_height_m
            + (t3 - 2.0 * t2 + t) * self.start_tangent_m
            + (-2.0 * t3 + 3.0 * t2) * self.end_height_m
            + (t3 - t2) * self.end_tangent_m
    }
    pub fn tangent_at(self, t: f64) -> f64 {
        (6.0 * t * t - 6.0 * t) * self.start_height_m
            + (3.0 * t * t - 4.0 * t + 1.0) * self.start_tangent_m
            + (-6.0 * t * t + 6.0 * t) * self.end_height_m
            + (3.0 * t * t - 2.0 * t) * self.end_tangent_m
    }
    pub fn is_flat(self) -> bool {
        self.start_height_m == self.end_height_m
            && self.start_tangent_m == 0.0
            && self.end_tangent_m == 0.0
    }
    /// Exact Hermite restriction, not a newly flattened smoothstep between cut ends.
    pub fn clip(self, from: f64, to: f64) -> Option<Self> {
        if !from.is_finite() || !to.is_finite() || from < 0.0 || to > 1.0 || from >= to {
            return None;
        }
        Some(Self {
            start_height_m: self.height_at(from),
            end_height_m: self.height_at(to),
            start_tangent_m: self.tangent_at(from) * (to - from),
            end_tangent_m: self.tangent_at(to) * (to - from),
        })
    }
    pub fn maximum_gradient_per_mille(self, horizontal_length_m: f64) -> f64 {
        let a = 6.0 * self.start_height_m + 3.0 * self.start_tangent_m - 6.0 * self.end_height_m
            + 3.0 * self.end_tangent_m;
        let b = -6.0 * self.start_height_m - 4.0 * self.start_tangent_m + 6.0 * self.end_height_m
            - 2.0 * self.end_tangent_m;
        if !a.is_finite() || !b.is_finite() {
            return f64::INFINITY;
        }
        let mut maximum = self.start_tangent_m.abs().max(self.end_tangent_m.abs());
        if a != 0.0 {
            let t = -b / (2.0 * a);
            if t > 0.0 && t < 1.0 {
                maximum = maximum.max(self.tangent_at(t).abs());
            }
        }
        1000.0 * maximum / horizontal_length_m
    }
    pub fn is_valid(self, horizontal_length_m: f64, track_type: TrackType) -> bool {
        self.start_tangent_m.is_finite()
            && self.end_tangent_m.is_finite()
            && self.start_height_m.is_finite()
            && self.end_height_m.is_finite()
            && horizontal_length_m.is_finite()
            && horizontal_length_m > 0.0
            && self.maximum_gradient_per_mille(horizontal_length_m)
                <= track_type.maximum_construction_gradient_per_mille() + 1e-9
    }
}
impl Alignment {
    pub fn section_lengths_m(&self) -> Vec<f64> {
        if self.points.len() < 2 {
            return vec![];
        }
        let anchors = self
            .anchor_indices
            .clone()
            .unwrap_or_else(|| vec![0, self.points.len() - 1]);
        anchors
            .windows(2)
            .map(|range| {
                self.points[range[0]..=range[1]]
                    .windows(2)
                    .map(|p| super::alignment::distance(p[0], p[1]))
                    .sum()
            })
            .collect()
    }
    pub fn height_and_grade_at_position(&self, position: f64) -> Option<(f64, f64)> {
        if !position.is_finite() || !(0.0..=1.0).contains(&position) {
            return None;
        }
        if !self.section_profiles.is_empty() {
            let lengths = self.section_lengths_m();
            let mut remaining = position * self.length_m();
            for (index, length) in lengths.iter().enumerate() {
                if remaining <= *length + 1e-8 || index + 1 == lengths.len() {
                    let p = self.section_profiles.get(index)?;
                    let t = (remaining / length).clamp(0., 1.);
                    return Some((p.height_at(t), p.tangent_at(t) / length));
                }
                remaining -= length;
            }
            return None;
        }
        Some(match self.vertical_profile {
            Some(p) => (
                p.height_at(position),
                p.tangent_at(position) / self.length_m(),
            ),
            None => (self.section_infrastructure(0)?.level.height_m(), 0.),
        })
    }
    pub fn height_at_position_m(&self, position: f64) -> Option<f64> {
        self.height_and_grade_at_position(position).map(|v| v.0)
    }
    pub fn endpoint_height_m(&self, endpoint: AlignmentEndpoint) -> f64 {
        self.height_at_position_m(if endpoint == AlignmentEndpoint::Start {
            0.
        } else {
            1.
        })
        .expect("validated endpoint")
    }
    pub fn endpoint_inward_grade(&self, endpoint: AlignmentEndpoint) -> f64 {
        let start = endpoint == AlignmentEndpoint::Start;
        self.height_and_grade_at_position(if start { 0. } else { 1. })
            .expect("validated endpoint")
            .1
            * if start { 1. } else { -1. }
    }
    /// Whole platform footprint, not just centre or the alignment's global label.
    pub fn is_flat_over_position(&self, from: f64, to: f64) -> bool {
        if !from.is_finite() || !to.is_finite() || from < -1e-9 || to > 1. + 1e-9 || from > to {
            return false;
        }
        let from = from.clamp(0., 1.);
        let to = to.clamp(0., 1.);
        if self.section_profiles.is_empty() {
            return self.vertical_profile.is_none_or(|p| p.is_flat());
        }
        let total = self.length_m();
        let mut cursor = 0.;
        for (profile, length) in self.section_profiles.iter().zip(self.section_lengths_m()) {
            let end = cursor + length;
            if to * total > cursor + 1e-8 && from * total < end - 1e-8 && !profile.is_flat() {
                return false;
            }
            cursor = end;
        }
        true
    }
}

impl World {
    /// Construct new ramp geometry atomically; never silently raise existing track.
    pub fn build_alignment_ramp(
        &mut self,
        owner: CompanyId,
        name: String,
        points: Vec<GeoPoint>,
        track_type: TrackType,
        start: InfrastructureLevel,
        finish: InfrastructureLevel,
    ) -> Result<AlignmentId, WorldError> {
        let mut candidate = Self::new(self.snapshot().clone())?;
        let id = candidate.build_alignment_typed(owner, name, points, track_type)?;
        let alignment = candidate
            .data_mut()
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .expect("new alignment");
        alignment.vertical_profile = Some(VerticalProfile {
            start_tangent_m: 0.0,
            end_tangent_m: 0.0,
            start_height_m: start.height_m(),
            end_height_m: finish.height_m(),
        });
        alignment.infrastructure = vec![SectionInfrastructure {
            track_count: 1,
            level: start,
        }];
        candidate.data.validate()?;
        *self = candidate;
        Ok(id)
    }
}
