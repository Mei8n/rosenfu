//! Read-only experiment using the CURRENT core's water constraints, including
//! its floating point tolerances. Not a coverage certificate or build command.
use rosenfu_core::world::{
    validate_connected_water, CoverageBounds, EnvironmentSnapshot, GeoPoint, WaterPolygon,
    WorldDraft,
};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub enum NotProven {
    InvalidWorld,
    NoAffectedTrack,
    OutsideEvaluationDomain,
    WaterConstraintsNotProven,
}
/// Opaque result: contains no synthetic geometry, coverage or buildable flag.
/// Road, structural and persistence checks remain mandatory.
#[derive(Debug, PartialEq, Eq)]
pub struct WaterConstraintProof {
    _private: (),
}

/// Caller must supply the actual authoritative before/after candidate snapshots.
/// Compute the diff here, so a caller cannot accidentally omit another changed
/// disconnected component from an explicit list of IDs.
/// All connected existing tracks are traversed by the existing core validator.
/// Neither saved environment nor either input snapshot is changed or merged.
pub fn prove_water_changes(
    before: &WorldDraft,
    after: &WorldDraft,
) -> Result<WaterConstraintProof, NotProven> {
    before.validate().map_err(|_| NotProven::InvalidWorld)?;
    after.validate().map_err(|_| NotProven::InvalidWorld)?;
    let mut changed: BTreeSet<_> = after
        .alignments
        .iter()
        .filter(|a| !before.alignments.contains(a))
        .map(|a| a.id)
        .collect();
    for c in &after.alignment_connections {
        if !before.alignment_connections.contains(c) {
            changed.extend([c.a.alignment_id, c.b.alignment_id]);
        }
    }
    if changed.is_empty() {
        return Err(NotProven::NoAffectedTrack);
    }
    // Strict margin keeps every vertex and interpolated segment/zero point away
    // from synthetic polygon edges, including half-open coverage boundaries.
    if after.alignments.iter().flat_map(|a| &a.points).any(|p| {
        !p.latitude.is_finite()
            || !p.longitude.is_finite()
            || p.longitude <= -179.
            || p.longitude >= 179.
            || p.latitude <= -84.
            || p.latitude >= 84.
    }) {
        return Err(NotProven::OutsideEvaluationDomain);
    }
    let water = all_water_assumption();
    water.validate().map_err(|_| NotProven::InvalidWorld)?;
    validate_connected_water(after, &changed.iter().copied().collect::<Vec<_>>(), &water)
        .map_err(|_| NotProven::WaterConstraintsNotProven)?;
    Ok(WaterConstraintProof { _private: () })
}

// NEVER export, persist, merge, serialize or display this synthetic snapshot.
// It asserts a hypothetical worst case, not anything about provider coverage.
fn all_water_assumption() -> EnvironmentSnapshot {
    let bounds = CoverageBounds {
        west: -180.,
        east: 180.,
        south: -85.,
        north: 85.,
    };
    let p = |longitude, latitude| GeoPoint {
        longitude,
        latitude,
    };
    EnvironmentSnapshot {
        revision: "private-counterfactual-all-water".into(),
        road_coverage: vec![],
        roads: vec![],
        water_coverage: vec![bounds],
        water: vec![WaterPolygon {
            coverage: Some(vec![bounds]),
            rings: vec![vec![
                p(-180., -85.),
                p(180., -85.),
                p(180., 85.),
                p(-180., 85.),
                p(-180., -85.),
            ]],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosenfu_core::world::*;
    fn p(m: f64) -> GeoPoint {
        GeoPoint {
            latitude: 0.,
            longitude: (m / 6_371_000.).to_degrees(),
        }
    }
    fn setup() -> World {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        w.add_company(player, "c".into()).unwrap();
        w
    }
    fn add(w: &mut World, start: f64, end: f64) -> AlignmentId {
        w.build_alignment(
            w.snapshot().companies[0].id,
            "a".into(),
            vec![p(start), p(end)],
        )
        .unwrap()
    }
    fn height(a: &mut Alignment, h: f64) {
        a.vertical_profile = Some(VerticalProfile {
            start_height_m: h,
            end_height_m: h,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        });
    }
    fn candidate(length: f64, h: f64) -> (WorldDraft, WorldDraft) {
        let mut w = setup();
        let before = w.snapshot().clone();
        add(&mut w, 0., length);
        let mut after = w.snapshot().clone();
        height(&mut after.alignments[0], h);
        after.validate().unwrap();
        (before, after)
    }
    fn link(a: AlignmentId, b: AlignmentId) -> AlignmentConnection {
        AlignmentConnection {
            a: AlignmentEndpointRef {
                alignment_id: a,
                endpoint: AlignmentEndpoint::End,
            },
            b: AlignmentEndpointRef {
                alignment_id: b,
                endpoint: AlignmentEndpoint::Start,
            },
        }
    }
    #[test]
    fn current_core_boundary_and_deep_exemption() {
        for (length, h, pass) in [
            (30_000., 6., true),
            (30_000.01, 6., false),
            (40_000., -6., false),
            (40_000., -12., true),
            (40_000., -18., true),
            (1000., 0., false),
        ] {
            let (before, after) = candidate(length, h);
            assert_eq!(
                prove_water_changes(&before, &after).is_ok(),
                pass,
                "{length}/{h}"
            );
        }
    }
    #[test]
    fn interior_ground_tangency_is_rejected() {
        let (before, mut after) = candidate(10_000., 6.);
        after.alignments[0]
            .vertical_profile
            .as_mut()
            .unwrap()
            .start_tangent_m = -24.;
        after.alignments[0]
            .vertical_profile
            .as_mut()
            .unwrap()
            .end_tangent_m = 24.;
        after.validate().unwrap();
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::WaterConstraintsNotProven)
        );
    }
    #[test]
    fn point_touching_minus_twelve_does_not_reset_limit() {
        let (before, mut after) = candidate(40_000., -11.);
        after.alignments[0]
            .vertical_profile
            .as_mut()
            .unwrap()
            .start_tangent_m = -4.;
        after.alignments[0]
            .vertical_profile
            .as_mut()
            .unwrap()
            .end_tangent_m = 4.;
        after.validate().unwrap();
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::WaterConstraintsNotProven)
        );
    }
    #[test]
    fn existing_connected_edge_is_included() {
        let mut w = setup();
        let a = add(&mut w, 0., 20_000.);
        let mut before = w.snapshot().clone();
        height(&mut before.alignments[0], 6.);
        w.replace(before.clone()).unwrap();
        let b = add(&mut w, 20_000., 40_000.);
        let mut after = w.snapshot().clone();
        height(&mut after.alignments[1], 6.);
        after.alignment_connections.push(link(a, b));
        after.validate().unwrap();
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::WaterConstraintsNotProven)
        );
    }
    #[test]
    fn disconnected_changed_ground_track_cannot_be_omitted() {
        let mut w = setup();
        let before = w.snapshot().clone();
        add(&mut w, 0., 100.);
        add(&mut w, 1000., 2000.);
        let mut after = w.snapshot().clone();
        height(&mut after.alignments[0], 6.);
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::WaterConstraintsNotProven)
        );
    }
    #[test]
    fn saved_coverage_is_ignored_and_worlds_are_unchanged() {
        let (before, mut after) = candidate(1000., 6.);
        let expected = prove_water_changes(&before, &after);
        after.environment = Some(EnvironmentSnapshot {
            revision: "observations-only".into(),
            ..Default::default()
        });
        let copy = after.clone();
        assert_eq!(prove_water_changes(&before, &after), expected);
        assert_eq!(after, copy);
        let bounds = CoverageBounds {
            west: -1.,
            east: 1.,
            south: -1.,
            north: 1.,
        };
        after
            .environment
            .as_mut()
            .unwrap()
            .water_coverage
            .push(bounds);
        let copy = after.clone();
        assert_eq!(prove_water_changes(&before, &after), expected);
        assert_eq!(after, copy);
        assert!(before.environment.is_none());
    }
    #[test]
    fn invalid_profile_no_change_and_unknown_ids_fail_closed() {
        let (before, mut after) = candidate(1000., 6.);
        assert_eq!(
            prove_water_changes(&before, &before),
            Err(NotProven::NoAffectedTrack)
        );
        after.alignments[0]
            .vertical_profile
            .as_mut()
            .unwrap()
            .start_height_m = f64::NAN;
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::InvalidWorld)
        );
        let (_, mut after) = candidate(1000., 6.);
        after
            .alignment_connections
            .push(link(after.alignments[0].id, AlignmentId(999)));
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::InvalidWorld)
        );
    }
    #[test]
    fn closed_ring_is_counted_once_across_its_connection() {
        for (radius, pass) in [(4000., true), (6000., false)] {
            let mut w = setup();
            let before = w.snapshot().clone();
            let mut points: Vec<_> = (0..144)
                .map(|i| {
                    let t = std::f64::consts::TAU * i as f64 / 144.;
                    GeoPoint {
                        longitude: p(radius * t.cos()).longitude,
                        latitude: p(radius * t.sin()).longitude,
                    }
                })
                .collect();
            points.push(points[0]);
            let id = w
                .build_alignment(w.snapshot().companies[0].id, "ring".into(), points)
                .unwrap();
            let mut after = w.snapshot().clone();
            height(&mut after.alignments[0], 6.);
            after.alignment_connections.push(link(id, id));
            after.validate().unwrap();
            assert_eq!(prove_water_changes(&before, &after).is_ok(), pass);
        }
    }
    #[test]
    fn anchor_profiles_override_ground_infrastructure() {
        let mut w = setup();
        let before = w.snapshot().clone();
        w.build_alignment(
            w.snapshot().companies[0].id,
            "anchors".into(),
            vec![p(0.), p(10_000.), p(20_000.)],
        )
        .unwrap();
        let mut after = w.snapshot().clone();
        let a = &mut after.alignments[0];
        a.anchor_indices = Some(vec![0, 1, 2]);
        a.section_profiles = vec![
            VerticalProfile {
                start_height_m: 6.,
                end_height_m: 6.,
                start_tangent_m: 0.,
                end_tangent_m: 0.
            };
            2
        ];
        after.validate().unwrap();
        assert!(prove_water_changes(&before, &after).is_ok());
    }
    #[test]
    fn connection_only_edit_checks_both_existing_tracks() {
        let mut w = setup();
        let a = add(&mut w, 0., 20_000.);
        let b = add(&mut w, 20_000., 40_000.);
        let mut before = w.snapshot().clone();
        for a in &mut before.alignments {
            height(a, 6.);
        }
        before.validate().unwrap();
        let mut after = before.clone();
        after.alignment_connections.push(link(a, b));
        after.validate().unwrap();
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::WaterConstraintsNotProven)
        );
    }
    #[test]
    fn synthetic_domain_boundary_is_rejected() {
        let (before, mut after) = candidate(1000., 6.);
        for p in &mut after.alignments[0].points {
            p.latitude = 84.;
        }
        after.validate().unwrap();
        assert_eq!(
            prove_water_changes(&before, &after),
            Err(NotProven::OutsideEvaluationDomain)
        );
    }
    #[test]
    fn every_sampled_water_subset_preserves_success() {
        let (before, after) = candidate(30_000., 6.);
        assert!(prove_water_changes(&before, &after).is_ok());
        for mask in 0..128 {
            let mut d = all_water_assumption();
            d.water.clear();
            for i in 0..7 {
                if mask & (1 << i) != 0 {
                    let x0 = p(i as f64 * 5000.).longitude;
                    let x1 = p((i + 1) as f64 * 5000.).longitude;
                    d.water.push(WaterPolygon {
                        coverage: None,
                        rings: vec![vec![
                            GeoPoint {
                                longitude: x0,
                                latitude: -1.,
                            },
                            GeoPoint {
                                longitude: x1,
                                latitude: -1.,
                            },
                            GeoPoint {
                                longitude: x1,
                                latitude: 1.,
                            },
                            GeoPoint {
                                longitude: x0,
                                latitude: 1.,
                            },
                            GeoPoint {
                                longitude: x0,
                                latitude: -1.,
                            },
                        ]],
                    });
                }
            }
            d.validate().unwrap();
            assert!(validate_connected_water(&after, &[after.alignments[0].id], &d).is_ok());
        }
    }
}
