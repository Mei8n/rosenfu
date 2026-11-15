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
    /// Signed grade pointing into the alignment from the specified endpoint.
    pub fn endpoint_inward_grade(&self, endpoint: AlignmentEndpoint) -> f64 {
        self.vertical_profile.map_or(0.0, |p| {
            if endpoint == AlignmentEndpoint::Start {
                p.start_tangent_m / self.length_m()
            } else {
                -p.end_tangent_m / self.length_m()
            }
        })
    }

    /// Actual height along horizontal arc length, including inside a ramp.
    /// Callers must use this, not categorical level, for crossing-clearance checks.
    pub fn height_at_position_m(&self, position: f64) -> Option<f64> {
        if !position.is_finite() || !(0.0..=1.0).contains(&position) {
            return None;
        }
        Some(match self.vertical_profile {
            Some(profile) => profile.height_at(position),
            None => self.section_infrastructure(0)?.level.height_m(),
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
                .height_m(),
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
