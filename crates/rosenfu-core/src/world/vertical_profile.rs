//! Construction-only vertical geometry. It never alters vehicle speed.
use super::*;

/// Flat-reference sandbox assumption, not a user-approved physical standard.
/// Persisted profiles store metres, independently of future layer spacing.
pub const PROVISIONAL_LEVEL_SPACING_M: f64 = 6.0;

impl InfrastructureLevel {
    pub fn provisional_height_m(self) -> f64 {
        let index = match self {
            Self::Ground => 0,
            Self::Elevated => 1,
            Self::Elevated2 => 2,
            Self::Elevated3 => 3,
            Self::Underground => -1,
            Self::Underground2 => -2,
            Self::Underground3 => -3,
        };
        f64::from(index) * PROVISIONAL_LEVEL_SPACING_M
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

/// Cubic smoothstep over horizontal arc length: flat tangent at both ends,
/// continuous grade throughout. Maximum grade occurs at the midpoint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerticalProfile {
    pub start_height_m: f64,
    pub end_height_m: f64,
}
impl VerticalProfile {
    pub fn height_at(self, position: f64) -> f64 {
        self.start_height_m
            + (self.end_height_m - self.start_height_m)
                * position
                * position
                * (3.0 - 2.0 * position)
    }
    pub fn maximum_gradient_per_mille(self, horizontal_length_m: f64) -> f64 {
        1500.0 * (self.end_height_m - self.start_height_m).abs() / horizontal_length_m
    }
    pub fn is_valid(self, horizontal_length_m: f64, track_type: TrackType) -> bool {
        self.start_height_m.is_finite()
            && self.end_height_m.is_finite()
            && horizontal_length_m.is_finite()
            && horizontal_length_m > 0.0
            && self.maximum_gradient_per_mille(horizontal_length_m)
                <= track_type.maximum_construction_gradient_per_mille() + 1e-9
    }
}
impl Alignment {
    /// Actual height along horizontal arc length, including inside a ramp.
    /// Callers must use this, not categorical level, for crossing-clearance checks.
    pub fn height_at_position_m(&self, position: f64) -> Option<f64> {
        if !position.is_finite() || !(0.0..=1.0).contains(&position) {
            return None;
        }
        Some(match self.vertical_profile {
            Some(profile) => profile.height_at(position),
            None => self.section_infrastructure(0)?.level.provisional_height_m(),
        })
    }
    /// Actual vertical height at an endpoint, used for physical link validation.
    pub fn endpoint_height_m(&self, endpoint: AlignmentEndpoint) -> f64 {
        let position = if endpoint == AlignmentEndpoint::Start {
            0.0
        } else {
            1.0
        };
        match self.vertical_profile {
            Some(profile) => profile.height_at(position),
            None => self
                .section_infrastructure(if endpoint == AlignmentEndpoint::Start {
                    0
                } else {
                    self.section_count() - 1
                })
                .expect("validated section")
                .level
                .provisional_height_m(),
        }
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
        let mut candidate = Self::new(self.data.clone())?;
        let id = candidate.build_alignment_typed(owner, name, points, track_type)?;
        let alignment = candidate
            .data
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .expect("new alignment");
        alignment.vertical_profile = Some(VerticalProfile {
            start_height_m: start.provisional_height_m(),
            end_height_m: finish.provisional_height_m(),
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
