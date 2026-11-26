use rosenfu_core::operation::{
    OperationRuntime, RunLeg, Section, ServiceRunSpec, Timetable, Vehicle,
};
use rosenfu_core::world::*;

fn sample(kind: TrackType) -> (World, CompanyId, AlignmentId) {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = world
        .build_alignment_typed(
            c,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.01,
                },
            ],
            kind,
        )
        .unwrap();
    (world, c, a)
}

#[test]
fn legacy_defaults_and_explicit_speed_roundtrip() {
    for (kind, expected) in [
        (TrackType::Normal, 120.0),
        (TrackType::HighSpeed, 300.0),
        (TrackType::Tram, 50.0),
    ] {
        let (mut world, _, id) = sample(kind);
        assert_eq!(
            world.snapshot().alignments[0].effective_design_speed_kmh(),
            expected
        );
        let old = serde_json::to_value(world.snapshot()).unwrap();
        assert!(old["alignments"][0].get("design_speed_kmh").is_none());
        let restored: WorldDraft = serde_json::from_value(old).unwrap();
        assert_eq!(World::new(restored).unwrap(), world);
        world.set_alignment_design_speed(id, 90.0).unwrap();
        let restored: WorldDraft =
            serde_json::from_slice(&serde_json::to_vec(world.snapshot()).unwrap()).unwrap();
        assert_eq!(World::new(restored).unwrap(), world);
    }
}

#[test]
fn invalid_speed_is_atomic_and_extension_preserves_cap() {
    let (mut world, owner, id) = sample(TrackType::Normal);
    world.set_alignment_design_speed(id, 95.0).unwrap();
    for speed in [0.0, -1.0, 600.1, f64::NAN, f64::INFINITY] {
        let before = world.clone();
        assert!(world.set_alignment_design_speed(id, speed).is_err());
        assert_eq!(world, before);
    }
    world
        .extend_alignment(
            owner,
            id,
            AlignmentEndpoint::End,
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.01,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.02,
                },
            ],
        )
        .unwrap();
    assert_eq!(world.snapshot().alignments[0].design_speed_kmh, Some(95.0));
}

#[test]
fn acceleration_and_design_speed_have_different_benefits_on_short_and_long_runs() {
    let arrival = |length_m, track_cap, acceleration_kmh_per_s, design_kmh| {
        let mut runtime = OperationRuntime::new(vec![ServiceRunSpec {
            service_id: "1".into(),
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 1,
            },
            vehicle: Vehicle {
                acceleration_mps2: acceleration_kmh_per_s / 3.6,
                max_speed_kmh: design_kmh,
                braking_mps2: 1.0,
                length_m: 40.0,
            },
            legs: vec![RunLeg {
                from: "A".into(),
                to: "B".into(),
                dwell_ms: 0,
                sections: vec![Section {
                    length_m,
                    speed_limit_kmh: track_cap,
                    resources: vec![],
                }],
            }],
        }])
        .unwrap();
        let advanced = runtime.advance_to(300_000, 8).unwrap();
        assert!(advanced.reached_target);
        assert_eq!(advanced.arrivals.len(), 1);
        assert_eq!(advanced.arrivals[0].station_id, "B");
        advanced.arrivals[0].at_ms
    };
    // Same braking and length isolate acceleration/speed. The first pair is
    // inspired by published Hanshin performance; the 160 km/h car is fictional.
    let run = |length, cap| {
        [(4.0, 110.0), (2.5, 110.0), (2.5, 160.0)]
            .map(|(acceleration, speed)| arrival(length, cap, acceleration, speed))
    };
    let short = run(400.0, 160.0);
    let long = run(5_000.0, 160.0);
    let capped = run(5_000.0, 110.0);
    assert!(short[0] < short[1]);
    assert_eq!(
        short[1], short[2],
        "neither low-acceleration train reaches its cap"
    );
    assert!(long[2] < long[0] && long[0] < long[1]);
    assert_eq!(
        capped[1], capped[2],
        "the infrastructure cap overrides design speed"
    );

    // Independent constant acceleration/cruise/braking reference, in ms.
    // A 2 ms tolerance allows integer rounding, not a change to the model.
    for (observed, expected) in [
        (short, [38_987, 44_181, 44_181]),
        (long, [192_664, 200_914, 166_722]),
        (capped, [192_664, 200_914, 200_914]),
    ] {
        for (actual, expected) in observed.into_iter().zip(expected) {
            assert!(actual.abs_diff(expected) <= 2, "{actual} vs {expected} ms");
        }
    }
}
