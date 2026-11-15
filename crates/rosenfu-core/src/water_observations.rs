//! Source observations for an already validated construction candidate.
//! This never certifies dry land, complete coverage, or permission to build.
use crate::world::{AlignmentId, GeoPoint, InfrastructureLevel, WorldDraft};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterAreaObservation {
    pub source_key: String,
    pub observation_id: String,
    pub polygon_part: usize,
    pub source_url: String,
    pub source_revision: String,
    pub source_sha256: String,
    pub tags: BTreeMap<String, String>,
    pub rings: Vec<Vec<GeoPoint>>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterInterpretation {
    /// The pinned ocean/coastline product claims an open sea surface, not a survey.
    MappedMarineSurface,
    /// A basin may be dry, seasonal or multi-use. Never infer dry land either.
    BasinUnresolved,
    CoveredOrHeightUnresolved,
    InlandSurfaceUnresolved,
}
#[derive(Debug, PartialEq, Serialize)]
pub struct ObservedGroundContact {
    pub alignment_id: AlignmentId,
    pub source_key: String,
    pub observation_id: String,
    pub polygon_part: usize,
    pub interpretation: WaterInterpretation,
}
#[derive(Debug, PartialEq, Serialize)]
pub struct GroundWaterObservations {
    /// Always unknown. No-hit is not evidence of dry land.
    pub coverage: UnknownCoverage,
    pub contacts: Vec<ObservedGroundContact>,
    /// Non-flat or non-ground profiles need the full physical water evaluator.
    pub unevaluated_profiles: Vec<AlignmentId>,
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownCoverage {
    Unknown,
}
fn interpretation(area: &WaterAreaObservation) -> WaterInterpretation {
    let tag = |key: &str| area.tags.get(key).map(String::as_str);
    if ["covered", "tunnel", "bridge"]
        .iter()
        .any(|key| tag(key).is_some_and(|value| value != "no"))
        || ["layer", "level", "ele"]
            .iter()
            .any(|key| tag(key).is_some_and(|value| value != "0"))
    {
        return WaterInterpretation::CoveredOrHeightUnresolved;
    }
    if tag("landuse") == Some("basin") || tag("water") == Some("basin") || tag("basin").is_some() {
        return WaterInterpretation::BasinUnresolved;
    }
    if tag("source_dataset") == Some("marine-water-polygons")
        && area.source_url
            == "https://osmdata.openstreetmap.de/download/water-polygons-split-4326.zip"
    {
        WaterInterpretation::MappedMarineSurface
    } else {
        // Absence of a height/covered tag does not verify a ground surface.
        WaterInterpretation::InlandSurfaceUnresolved
    }
}
fn point_valid(p: GeoPoint) -> bool {
    p.longitude.is_finite()
        && p.latitude.is_finite()
        && p.longitude.abs() <= 180.
        && p.latitude.abs() <= 85.
}
fn side(a: GeoPoint, b: GeoPoint, p: GeoPoint) -> f64 {
    (b.longitude - a.longitude) * (p.latitude - a.latitude)
        - (b.latitude - a.latitude) * (p.longitude - a.longitude)
}
fn on_edge(a: GeoPoint, b: GeoPoint, p: GeoPoint) -> bool {
    side(a, b, p).abs() <= 1e-12
        && p.longitude >= a.longitude.min(b.longitude) - 1e-10
        && p.longitude <= a.longitude.max(b.longitude) + 1e-10
        && p.latitude >= a.latitude.min(b.latitude) - 1e-10
        && p.latitude <= a.latitude.max(b.latitude) + 1e-10
}
fn intersects(a: GeoPoint, b: GeoPoint, c: GeoPoint, d: GeoPoint) -> bool {
    on_edge(a, b, c)
        || on_edge(a, b, d)
        || on_edge(c, d, a)
        || on_edge(c, d, b)
        || ((side(a, b, c) > 0.) != (side(a, b, d) > 0.)
            && (side(c, d, a) > 0.) != (side(c, d, b) > 0.))
}
fn inside(p: GeoPoint, ring: &[GeoPoint]) -> bool {
    let mut result = false;
    for pair in ring.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if (a.latitude > p.latitude) != (b.latitude > p.latitude)
            && p.longitude
                < (b.longitude - a.longitude) * (p.latitude - a.latitude)
                    / (b.latitude - a.latitude)
                    + a.longitude
        {
            result = !result;
        }
    }
    result
}
fn validate_area(area: &WaterAreaObservation, budget: &mut usize) -> Result<(), String> {
    if area.source_key.is_empty()
        || area.observation_id.is_empty()
        || area.source_revision.is_empty()
        || area.source_key.len() > 256
        || area.observation_id.len() > 1024
        || area.source_revision.len() > 4096
        || area.source_url.len() > 4096
        || !area.source_url.starts_with("https://")
        || area.source_sha256.len() != 64
        || !area
            .source_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || area.tags.len() > 128
        || area
            .tags
            .iter()
            .any(|(k, v)| k.is_empty() || k.len() > 1024 || v.len() > 16384)
        || area.rings.is_empty()
        || area.rings.len() > 1000
    {
        return Err("invalid water observation metadata".into());
    }
    for ring in &area.rings {
        if ring.len() < 4 || ring.first() != ring.last() || ring.iter().any(|p| !point_valid(*p)) {
            return Err("invalid water observation ring".into());
        }
        let west = ring
            .iter()
            .map(|p| p.longitude)
            .fold(f64::INFINITY, f64::min);
        let east = ring
            .iter()
            .map(|p| p.longitude)
            .fold(f64::NEG_INFINITY, f64::max);
        if east - west > 180. {
            return Err("antimeridian water geometry unsupported".into());
        }
        *budget = budget
            .checked_sub(ring.len())
            .ok_or("water coordinate limit")?;
    }
    Ok(())
}
fn contact(
    points: &[GeoPoint],
    rings: &[Vec<GeoPoint>],
    budget: &mut usize,
) -> Result<bool, String> {
    for segment in points.windows(2) {
        for ring in rings {
            for edge in ring.windows(2) {
                *budget = budget
                    .checked_sub(1)
                    .ok_or("water intersection work limit")?;
                if intersects(segment[0], segment[1], edge[0], edge[1]) {
                    return Ok(true);
                }
            }
        }
        if inside(segment[0], &rings[0]) && !rings[1..].iter().any(|ring| inside(segment[0], ring))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
/// Assess all changed candidate alignments, never a caller-selected subset.
/// Uses the core's anchor/profile model. Only wholly flat ground is evaluated;
/// transitions and elevated/underground paths are explicitly unevaluated.
/// Contacts include boundary/near-boundary contacts in stored lon/lat coordinates.
/// Rings require upstream topology validation: this checks structure and bounds,
/// not self-intersection or real-world accuracy. Source hashes are provenance;
/// authenticity must be established by the pinned delivery reader before calling.
/// No water/road snapshot or approval is written into either input world.
pub fn observe_ground_water(
    before: &WorldDraft,
    candidate: &WorldDraft,
    areas: &[WaterAreaObservation],
) -> Result<GroundWaterObservations, String> {
    before.validate().map_err(|e| e.to_string())?;
    candidate.validate().map_err(|e| e.to_string())?;
    if areas.len() > 10000 {
        return Err("water observation limit".into());
    }
    let mut coordinate_budget = 100000;
    for area in areas {
        validate_area(area, &mut coordinate_budget)?;
    }
    let mut result = GroundWaterObservations {
        coverage: UnknownCoverage::Unknown,
        contacts: vec![],
        unevaluated_profiles: vec![],
    };
    let mut work_budget = 2_000_000;
    let previous: BTreeMap<_, _> = before
        .alignments
        .iter()
        .map(|path| (path.id, path))
        .collect();
    for path in &candidate.alignments {
        if previous.get(&path.id).is_some_and(|old| *old == path) {
            continue;
        }
        let ground = if !path.section_profiles.is_empty() {
            path.section_profiles
                .iter()
                .all(|p| p.is_flat() && p.start_height_m == 0.)
        } else if let Some(profile) = path.vertical_profile {
            profile.is_flat() && profile.start_height_m == 0.
        } else {
            path.infrastructure
                .iter()
                .all(|s| s.level == InfrastructureLevel::Ground)
        };
        if !ground {
            result.unevaluated_profiles.push(path.id);
            continue;
        }
        if path.points.iter().any(|p| !point_valid(*p))
            || path
                .points
                .windows(2)
                .any(|p| (p[0].longitude - p[1].longitude).abs() > 180.)
        {
            return Err("unsupported ground path coordinates".into());
        }
        for area in areas {
            if contact(&path.points, &area.rings, &mut work_budget)? {
                if result.contacts.len() >= 1024 {
                    return Err("water contact output limit".into());
                }
                result.contacts.push(ObservedGroundContact {
                    alignment_id: path.id,
                    source_key: area.source_key.clone(),
                    observation_id: area.observation_id.clone(),
                    polygon_part: area.polygon_part,
                    interpretation: interpretation(area),
                });
            }
        }
    }
    Ok(result)
}
