//! Exact height-zone subdivision and bounded traversal of explicitly connected tracks.
use super::*;

struct PhysicalPath {
    points: Vec<GeoPoint>,
    levels: Vec<InfrastructureLevel>,
    ground: Vec<bool>,
    zero_points: Vec<GeoPoint>,
    lengths: Vec<f64>,
}

// Locate every cubic crossing, including a tangency at an interior extremum.
// The derivative divides Hermite into monotone intervals; bisection never relies
// on a fixed sampling grid that could miss a short ground/deep excursion.
fn height_cuts(p: VerticalProfile, height: f64) -> Vec<f64> {
    let a =
        6. * p.start_height_m + 3. * p.start_tangent_m - 6. * p.end_height_m + 3. * p.end_tangent_m;
    let b = -6. * p.start_height_m - 4. * p.start_tangent_m + 6. * p.end_height_m
        - 2. * p.end_tangent_m;
    let c = p.start_tangent_m;
    let mut extrema = vec![0., 1.];
    if a == 0. {
        if b != 0. {
            extrema.push(-c / b);
        }
    } else {
        let d = b * b - 4. * a * c;
        if d >= 0. {
            let q = -0.5 * (b + d.sqrt().copysign(b));
            if q != 0. {
                extrema.extend([q / a, c / q]);
            } else {
                extrema.push(-b / (2. * a));
            }
        }
    }
    extrema.retain(|t| t.is_finite() && *t >= 0. && *t <= 1.);
    extrema.sort_by(f64::total_cmp);
    extrema.dedup_by(|a, b| (*a - *b).abs() < 1e-14);
    let mut roots: Vec<_> = extrema
        .iter()
        .copied()
        .filter(|t| (p.height_at(*t) - height).abs() <= 1e-9)
        .collect();
    for span in extrema.windows(2) {
        let (mut lo, mut hi) = (span[0], span[1]);
        let sign = p.height_at(lo) - height;
        if sign * (p.height_at(hi) - height) >= 0. {
            continue;
        }
        for _ in 0..60 {
            let mid = (lo + hi) / 2.;
            if (p.height_at(mid) - height) * sign > 0. {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        roots.push((lo + hi) / 2.);
    }
    roots
}

fn physical_path(alignment: &Alignment) -> Option<PhysicalPath> {
    if alignment.points.len() < 2 {
        return None;
    }
    let anchors = alignment
        .anchor_indices
        .clone()
        .unwrap_or_else(|| vec![0, alignment.points.len() - 1]);
    if anchors.first() != Some(&0)
        || anchors.last() != Some(&(alignment.points.len() - 1))
        || anchors
            .windows(2)
            .any(|p| p[0] >= p[1] || p[1] >= alignment.points.len())
        || (!alignment.section_profiles.is_empty()
            && alignment.section_profiles.len() != anchors.len() - 1)
    {
        return None;
    }
    let lengths = alignment.section_lengths_m();
    let total: f64 = lengths.iter().sum();
    if !total.is_finite() || total <= 0. || lengths.iter().any(|l| !l.is_finite() || *l <= 0.) {
        return None;
    }
    let mut path = PhysicalPath {
        points: vec![alignment.points[0]],
        levels: vec![],
        ground: vec![],
        zero_points: vec![],
        lengths: vec![],
    };
    let mut section_start = 0.;
    for (index, range) in anchors.windows(2).enumerate() {
        let length = lengths[index];
        let fallback = alignment
            .section_infrastructure(index)
            .unwrap_or_default()
            .level;
        let profile = alignment
            .section_profiles
            .get(index)
            .copied()
            .or_else(|| {
                alignment.vertical_profile.and_then(|p| {
                    p.clip(
                        section_start / total,
                        ((section_start + length) / total).min(1.),
                    )
                })
            })
            .unwrap_or(VerticalProfile {
                start_height_m: fallback.height_m(),
                end_height_m: fallback.height_m(),
                start_tangent_m: 0.,
                end_tangent_m: 0.,
            });
        if !profile.is_valid(length, alignment.track_type) {
            return None;
        }
        let mut offset = 0.;
        for pair in alignment.points[range[0]..=range[1]].windows(2) {
            let segment_length = crate::world::alignment::distance(pair[0], pair[1]);
            let from = (offset / length).clamp(0., 1.);
            let to = ((offset + segment_length) / length).clamp(0., 1.);
            let p = profile.clip(from, to)?;
            let mut cuts = vec![0., 1.];
            let zeros = height_cuts(p, 0.);
            path.zero_points
                .extend(zeros.iter().map(|t| mix(pair[0], pair[1], *t)));
            cuts.extend(zeros);
            cuts.extend(height_cuts(p, -2. * LEVEL_SPACING_M));
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
            for span in cuts.windows(2) {
                let h = p.height_at((span[0] + span[1]) / 2.);
                let ground = p.is_flat() && h.abs() <= 1e-6;
                let level = if ground {
                    InfrastructureLevel::Ground
                } else if h <= -2. * LEVEL_SPACING_M {
                    InfrastructureLevel::Underground2
                } else {
                    InfrastructureLevel::Elevated
                };
                path.points.push(mix(pair[0], pair[1], span[1]));
                path.lengths
                    .push(metres(pair[0], pair[1]) * (span[1] - span[0]));
                path.levels.push(level);
                path.ground.push(ground);
            }
            offset += segment_length;
        }
        section_start += length;
    }
    Some(path)
}

pub fn assess_profile_alignment_environment(
    alignment: &Alignment,
    data: &EnvironmentSnapshot,
) -> EnvironmentAssessment {
    let Some(path) = physical_path(alignment) else {
        return EnvironmentAssessment {
            crossings: vec![],
            roads_ready: false,
            water_ready: false,
            unknown_road_height: false,
            ground_water: false,
            longest_water_m: 0.,
            water_limit_exceeded: false,
        };
    };
    let mut result = assess_environment_sections(
        &path.points,
        &path.levels,
        &path.ground,
        Some(&path.lengths),
        data,
    );
    // A sloping track crossing height zero in water is prohibited even if the
    // zero-height point has no length and no sampled interval midpoint hits it.
    result.ground_water |= path
        .zero_points
        .iter()
        .any(|p| water_contains(*p, &data.water));
    result
}

#[derive(Clone, Copy, Default)]
struct WetRun {
    prefix: f64,
    suffix: f64,
    whole: bool,
}
fn wet_run(path: &PhysicalPath, data: &EnvironmentSnapshot, reverse: bool) -> WetRun {
    let mut pieces = vec![];
    for (i, pair) in path.points.windows(2).enumerate() {
        for span in water_cuts(pair[0], pair[1], data).windows(2) {
            let wet = path.levels[i] != InfrastructureLevel::Underground2
                && water_contains(mix(pair[0], pair[1], (span[0] + span[1]) / 2.), &data.water);
            pieces.push((wet, path.lengths[i] * (span[1] - span[0])));
        }
    }
    if reverse {
        pieces.reverse();
    }
    WetRun {
        prefix: pieces.iter().take_while(|p| p.0).map(|p| p.1).sum(),
        suffix: pieces.iter().rev().take_while(|p| p.0).map(|p| p.1).sum(),
        whole: pieces.iter().all(|p| p.0),
    }
}

/// Examine every edge-simple route through actual endpoint connections. No
/// arbitrary branch is discarded; a short wholly wet ring counts once. The
/// explicit search budget rejects complex graphs rather than certifying a
/// partially searched component. No public fetches or unbounded recursion occur.
pub fn validate_connected_water(
    world: &WorldDraft,
    changed: &[AlignmentId],
    data: &EnvironmentSnapshot,
) -> Result<(), WorldError> {
    let mut connections = BTreeSet::new();
    for c in &world.alignment_connections {
        let mut pair = [
            (c.a.alignment_id, c.a.endpoint == AlignmentEndpoint::End),
            (c.b.alignment_id, c.b.endpoint == AlignmentEndpoint::End),
        ];
        pair.sort();
        if !connections.insert(pair) {
            return Err(WorldError::InvalidEnvironment);
        }
    }
    let mut relevant: BTreeSet<_> = changed.iter().copied().collect();
    let mut pending = changed.to_vec();
    while let Some(id) = pending.pop() {
        for c in &world.alignment_connections {
            let next = if c.a.alignment_id == id {
                Some(c.b.alignment_id)
            } else if c.b.alignment_id == id {
                Some(c.a.alignment_id)
            } else {
                None
            };
            if let Some(next) = next {
                if relevant.insert(next) {
                    pending.push(next);
                }
            }
        }
    }
    let mut runs = std::collections::BTreeMap::new();
    for a in world.alignments.iter().filter(|a| relevant.contains(&a.id)) {
        let assessment = assess_profile_alignment_environment(a, data);
        if !assessment.water_ready || assessment.ground_water || assessment.water_limit_exceeded {
            return Err(WorldError::InvalidEnvironment);
        }
        let path = physical_path(a).ok_or(WorldError::InvalidEnvironment)?;
        runs.insert((a.id, false), wet_run(&path, data, false));
        runs.insert((a.id, true), wet_run(&path, data, true));
    }
    let mut budget = 100_000usize;
    for (&start, &first) in &runs {
        let mut stack = vec![(start, first.suffix, BTreeSet::from([start.0]))];
        while let Some(((id, reverse), tail, used)) = stack.pop() {
            budget = budget
                .checked_sub(1)
                .ok_or(WorldError::InvalidEnvironment)?;
            let exit = if reverse {
                AlignmentEndpoint::Start
            } else {
                AlignmentEndpoint::End
            };
            for c in &world.alignment_connections {
                let next = if c.a.alignment_id == id && c.a.endpoint == exit {
                    Some(c.b)
                } else if c.b.alignment_id == id && c.b.endpoint == exit {
                    Some(c.a)
                } else {
                    None
                };
                let Some(next) = next else { continue };
                let key = (next.alignment_id, next.endpoint == AlignmentEndpoint::End);
                let run = runs.get(&key).ok_or(WorldError::InvalidEnvironment)?;
                if used.contains(&key.0) {
                    if key == start && !first.whole && tail + first.prefix > 30000. + 1e-6 {
                        return Err(WorldError::InvalidEnvironment);
                    }
                    continue;
                }
                if tail + run.prefix > 30000. + 1e-6 {
                    return Err(WorldError::InvalidEnvironment);
                }
                let mut visited = used.clone();
                visited.insert(key.0);
                if stack.len() >= 4096 || used.len() >= 4096 {
                    return Err(WorldError::InvalidEnvironment);
                }
                stack.push((
                    key,
                    if run.whole {
                        tail + run.suffix
                    } else {
                        run.suffix
                    },
                    visited,
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> GeoPoint {
        GeoPoint {
            longitude: x,
            latitude: y,
        }
    }
    fn fixture(length: f64) -> (World, AlignmentId, EnvironmentSnapshot) {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        let owner = w.add_company(player, "c".into()).unwrap();
        let id = w
            .build_alignment(owner, "a".into(), vec![p(0., 0.), p(length, 0.)])
            .unwrap();
        let bounds = CoverageBounds {
            west: -2.,
            east: 2.,
            south: -2.,
            north: 2.,
        };
        let d = EnvironmentSnapshot {
            revision: "test".into(),
            road_coverage: vec![bounds],
            water_coverage: vec![bounds],
            roads: vec![],
            water: vec![WaterPolygon {
                coverage: None,
                rings: vec![vec![
                    p(-1., -1.),
                    p(2., -1.),
                    p(2., 1.),
                    p(-1., 1.),
                    p(-1., -1.),
                ]],
            }],
        };
        (w, id, d)
    }
    fn profile(h: f64, t: f64) -> VerticalProfile {
        VerticalProfile {
            start_height_m: h,
            end_height_m: h,
            start_tangent_m: t,
            end_tangent_m: -t,
        }
    }
    #[test]
    fn interior_ground_tangency_and_crossing_are_not_missed() {
        let (w, _, d) = fixture(0.1);
        let mut a = w.snapshot().alignments[0].clone();
        a.vertical_profile = Some(profile(6., -24.)); // touches 0 only at t=0.5
        assert!(assess_profile_alignment_environment(&a, &d).ground_water);
        a.vertical_profile = Some(profile(-18., 80.)); // rises above zero then returns
        assert!(assess_profile_alignment_environment(&a, &d).ground_water);
        let mut dry_middle = d.clone();
        dry_middle.water[0].rings.push(vec![
            p(0.04, -0.1),
            p(0.06, -0.1),
            p(0.06, 0.1),
            p(0.04, 0.1),
            p(0.04, -0.1),
        ]);
        a.vertical_profile = Some(profile(6., -24.));
        assert!(!assess_profile_alignment_environment(&a, &dry_middle).ground_water);
    }
    #[test]
    fn deep_exemption_uses_actual_hermite_height() {
        let (w, _, d) = fixture(1.);
        let mut a = w.snapshot().alignments[0].clone();
        a.infrastructure = vec![SectionInfrastructure {
            track_count: 1,
            level: InfrastructureLevel::Underground2,
        }];
        a.vertical_profile = Some(profile(6., 0.));
        assert!(assess_profile_alignment_environment(&a, &d).water_limit_exceeded);
        a.vertical_profile = Some(profile(-18., 20.));
        assert!(!assess_profile_alignment_environment(&a, &d).water_limit_exceeded);
        a.vertical_profile = Some(profile(-18., 40.));
        assert!(assess_profile_alignment_environment(&a, &d).water_limit_exceeded);
    }
    #[test]
    fn actual_flat_zero_overrides_stale_category_for_crossings() {
        let (w, _, mut d) = fixture(0.1);
        let mut a = w.snapshot().alignments[0].clone();
        d.water.clear();
        a.infrastructure = vec![SectionInfrastructure {
            track_count: 1,
            level: InfrastructureLevel::Elevated,
        }];
        a.vertical_profile = Some(profile(0., 0.));
        d.roads.push(RoadWay {
            coverage: None,
            id: "1".into(),
            points: vec![p(0.05, -0.1), p(0.05, 0.1)],
            layer: None,
            level: None,
            bridge: None,
            tunnel: None,
        });
        assert_eq!(
            assess_profile_alignment_environment(&a, &d).crossings.len(),
            1
        );
    }
    #[test]
    fn all_branch_choices_are_checked_without_rejecting_legal_branch() {
        let (mut w, stem, d) = fixture(0.1);
        let owner = w.snapshot().alignments[0].owner;
        let short = w
            .build_alignment(owner, "b".into(), vec![p(0.1, 0.), p(0.2, 0.)])
            .unwrap();
        let long = w
            .build_alignment(owner, "c".into(), vec![p(0.1, 0.), p(0.1, 0.3)])
            .unwrap();
        let mut draft = w.snapshot().clone();
        for a in &mut draft.alignments {
            a.infrastructure = vec![SectionInfrastructure {
                track_count: 1,
                level: InfrastructureLevel::Elevated,
            }];
        }
        for id in [short, long] {
            draft.alignment_connections.push(AlignmentConnection {
                a: AlignmentEndpointRef {
                    alignment_id: stem,
                    endpoint: AlignmentEndpoint::End,
                },
                b: AlignmentEndpointRef {
                    alignment_id: id,
                    endpoint: AlignmentEndpoint::Start,
                },
            });
        }
        assert!(validate_connected_water(&draft, &[short], &d).is_err());
        draft
            .alignments
            .iter_mut()
            .find(|a| a.id == long)
            .unwrap()
            .points[1] = p(0.1, 0.1);
        assert!(validate_connected_water(&draft, &[short], &d).is_ok());
    }
    #[test]
    fn thirty_km_is_invariant_under_profile_zone_subdivision() {
        let (w, _, mut d) = fixture(0.1);
        let mut a = w.snapshot().alignments[0].clone();
        d.road_coverage = vec![CoverageBounds {
            west: -1.,
            east: 2.,
            south: 79.,
            north: 82.,
        }];
        d.water_coverage = d.road_coverage.clone();
        d.water[0].rings = vec![vec![
            p(-1., 79.),
            p(2., 79.),
            p(2., 82.),
            p(-1., 82.),
            p(-1., 79.),
        ]];
        a.vertical_profile = Some(profile(-6., -24.)); // touches -12m midway, splitting twice
        for (length, exceeded) in [(30000., false), (30000.01, true)] {
            let (mut lo, mut hi) = (0., 1.);
            for _ in 0..60 {
                let mid = (lo + hi) / 2.;
                if metres(p(0., 80.), p(mid, 80. + mid)) < length {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let delta = (lo + hi) / 2.;
            a.points = vec![p(0., 80.), p(delta, 80. + delta)];
            assert_eq!(
                assess_profile_alignment_environment(&a, &d).water_limit_exceeded,
                exceeded
            );
        }
        a.points.clear();
        assert!(!assess_profile_alignment_environment(&a, &d).buildable());
    }
}
