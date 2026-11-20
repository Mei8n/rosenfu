//! Source observations for an already validated construction candidate.
//! This never certifies dry land, complete coverage, or permission to build.
use crate::world::{
    Alignment, AlignmentId, GeoPoint, InfrastructureLevel, VerticalProfile, WorldDraft,
};
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
    /// Approximate zero-height roots, never certified ground locations.
    pub possible_ground_contacts: Vec<ObservedGroundContact>,
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
// Matches the core's horizontal arc-length metric (world/alignment.rs).
fn length(a: GeoPoint, b: GeoPoint) -> f64 {
    let x = (b.longitude - a.longitude).to_radians()
        * ((a.latitude + b.latitude) / 2.).to_radians().cos();
    let y = (b.latitude - a.latitude).to_radians();
    6_371_000. * x.hypot(y)
}
fn point_at(points: &[GeoPoint], position: f64) -> GeoPoint {
    let total: f64 = points.windows(2).map(|p| length(p[0], p[1])).sum();
    let mut remaining = total * position;
    for pair in points.windows(2) {
        let len = length(pair[0], pair[1]);
        if remaining <= len && len > 0. {
            let t = (remaining / len).clamp(0., 1.);
            return GeoPoint {
                longitude: pair[0].longitude + t * (pair[1].longitude - pair[0].longitude),
                latitude: pair[0].latitude + t * (pair[1].latitude - pair[0].latitude),
            };
        }
        remaining -= len;
    }
    *points.last().expect("validated path")
}
/// Split the cubic at derivative roots, then bracket zero on each monotone span.
/// Tangencies/near tangencies are candidates within 1e-9 m; neither these nor
/// the 64-step bisection results are exact ground-location certificates.
pub(crate) fn zero_candidates(p: VerticalProfile) -> Vec<f64> {
    let a =
        6. * p.start_height_m + 3. * p.start_tangent_m - 6. * p.end_height_m + 3. * p.end_tangent_m;
    let b = -6. * p.start_height_m - 4. * p.start_tangent_m + 6. * p.end_height_m
        - 2. * p.end_tangent_m;
    let c = p.start_tangent_m;
    let mut cuts = vec![0., 1.];
    if a == 0. {
        if b != 0. {
            cuts.push(-c / b);
        }
    } else {
        let discriminant = b * b - 4. * a * c;
        if discriminant >= 0. {
            // Stable quadratic formula, including the zero-double-root case.
            let q = -0.5 * (b + discriminant.sqrt().copysign(b));
            if q == 0. {
                cuts.push(-b / (2. * a));
            } else {
                cuts.extend([q / a, c / q]);
            }
        }
    }
    cuts.retain(|t| t.is_finite() && *t >= 0. && *t <= 1.);
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut roots: Vec<_> = cuts
        .iter()
        .copied()
        .filter(|t| p.height_at(*t).abs() <= 1e-9)
        .collect();
    for span in cuts.windows(2) {
        let (mut lo, mut hi) = (span[0], span[1]);
        let start = p.height_at(lo);
        let finish = p.height_at(hi);
        if start == 0. || finish == 0. || start.is_sign_positive() == finish.is_sign_positive() {
            continue;
        }
        for _ in 0..64 {
            let mid = (lo + hi) / 2.;
            let value = p.height_at(mid);
            if value == 0. {
                lo = mid;
                hi = mid;
                break;
            }
            if value.is_sign_positive() == start.is_sign_positive() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        roots.push((lo + hi) / 2.);
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup();
    roots
}
fn ground_pieces(path: &Alignment) -> (Vec<&[GeoPoint]>, Vec<GeoPoint>) {
    let anchors = path
        .anchor_indices
        .clone()
        .unwrap_or_else(|| vec![0, path.points.len() - 1]);
    let mut flat = vec![];
    let mut possible = vec![];
    let mut add = |points, profile: VerticalProfile| {
        if profile.is_flat() {
            if profile.start_height_m == 0. {
                flat.push(points);
            }
        } else {
            possible.extend(
                zero_candidates(profile)
                    .into_iter()
                    .map(|t| point_at(points, t)),
            );
        }
    };
    if !path.section_profiles.is_empty() {
        for (indices, profile) in anchors.windows(2).zip(&path.section_profiles) {
            add(&path.points[indices[0]..=indices[1]], *profile);
        }
    } else if let Some(profile) = path.vertical_profile {
        add(&path.points, profile);
    } else {
        for (i, indices) in anchors.windows(2).enumerate() {
            if path
                .section_infrastructure(i)
                .expect("validated section")
                .level
                == InfrastructureLevel::Ground
            {
                flat.push(&path.points[indices[0]..=indices[1]]);
            }
        }
    }
    (flat, possible)
}

/// Assess all changed candidate alignments, never a caller-selected subset.
/// Flat ground portions and approximate zero-height candidates are observed.
/// Non-wholly-ground paths remain unevaluated for complete physical constraints.
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
        possible_ground_contacts: vec![],
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
        }
        if path.points.iter().any(|p| !point_valid(*p))
            || path
                .points
                .windows(2)
                .any(|p| (p[0].longitude - p[1].longitude).abs() > 180.)
        {
            return Err("unsupported ground path coordinates".into());
        }
        let (flat, possible) = ground_pieces(path);
        for area in areas {
            let mut exact_contact = false;
            for points in &flat {
                if contact(points, &area.rings, &mut work_budget)? {
                    exact_contact = true;
                    break;
                }
            }
            let mut possible_contact = false;
            for point in &possible {
                if contact(&[*point, *point], &area.rings, &mut work_budget)? {
                    possible_contact = true;
                    break;
                }
            }
            if exact_contact || possible_contact {
                if result.contacts.len() + result.possible_ground_contacts.len() >= 1024 {
                    return Err("water contact output limit".into());
                }
                let output = if exact_contact {
                    &mut result.contacts
                } else {
                    &mut result.possible_ground_contacts
                };
                output.push(ObservedGroundContact {
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
