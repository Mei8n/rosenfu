use super::*;

pub(super) fn meeting_loop(
    separate_bays: bool,
) -> (
    World,
    [OperationServiceConfig; 2],
    [AlignmentId; 4],
    [StationId; 4],
) {
    let mut world = World::default();
    let player = world.add_player("Meet scenario".into()).unwrap();
    let owner = world.add_company(player, "Rail".into()).unwrap();
    let p = |longitude, dy| GeoPoint {
        latitude: 35. + dy,
        longitude,
    };
    let mut build = |name, points| world.build_alignment(owner, name, points).unwrap();
    let west = build(
        "West approach".into(),
        vec![p(139.500, 0.), p(139.504, 0.), p(139.510, 0.)],
    );
    let east = build(
        "East approach".into(),
        vec![p(139.522, 0.), p(139.528, 0.), p(139.532, 0.)],
    );
    let main = build("Eastbound bay".into(), vec![p(139.510, 0.), p(139.522, 0.)]);
    let side = build(
        "Westbound bay".into(),
        vec![
            p(139.510, 0.),
            p(139.512, 0.00006),
            p(139.515, 0.00016),
            p(139.517, 0.00016),
            p(139.520, 0.00006),
            p(139.522, 0.),
        ],
    );
    let mut d = world.snapshot().clone();
    for a in &mut d.alignments {
        if [west, east].contains(&a.id) {
            a.anchor_indices = Some(vec![0, 1, 2]);
            let count = if a.id == west { [2, 1] } else { [1, 2] };
            a.infrastructure = count
                .into_iter()
                .map(|track_count| SectionInfrastructure {
                    track_count,
                    ..Default::default()
                })
                .collect();
        }
    }
    world.replace(d).unwrap();
    let link = |a, b| AlignmentConnection {
        a: AlignmentEndpointRef {
            alignment_id: a,
            endpoint: End,
        },
        b: AlignmentEndpointRef {
            alignment_id: b,
            endpoint: Start,
        },
    };
    let wm = link(west, main);
    let me = link(main, east);
    let ws = link(west, side);
    let se = link(side, east);
    for l in [&wm, &me, &ws, &se] {
        world.connect_alignment_endpoints(owner, l.a, l.b).unwrap();
    }
    let a = world
        .place_station_on_alignment(owner, "A".into(), west, 0.15, 80., 10.)
        .unwrap();
    let b = world
        .place_station_on_alignment(owner, "B east bay".into(), main, 0.5, 80., 10.)
        .unwrap();
    let s = world
        .place_station_on_alignment(owner, "B west bay".into(), side, 0.5, 80., 10.)
        .unwrap();
    let c = world
        .place_station_on_alignment(owner, "C".into(), east, 0.85, 80., 10.)
        .unwrap();
    let rev = |l: &AlignmentConnection| AlignmentConnection { a: l.b, b: l.a };
    let westbound = if separate_bays {
        vec![rev(&se), rev(&ws)]
    } else {
        vec![rev(&me), rev(&wm)]
    };
    let both = [
        ("Eastbound", a, c, b, vec![wm, me]),
        (
            "Westbound",
            c,
            a,
            if separate_bays { s } else { b },
            westbound,
        ),
    ];
    let services = both.map(|(name, from, to, mid, connections)| {
        let legs = vec![RouteSegment::ConnectedAlignments {
            from,
            to,
            connections,
        }];
        let preview = world.preview_service_path(from, legs.clone()).unwrap();
        let stops = preview
            .occurrences
            .into_iter()
            .filter(|p| [from, mid, to].contains(&p.station_id))
            .map(|p| p.reference)
            .collect();
        let service_id = world
            .add_service_path(owner, name.into(), from, legs, stops)
            .unwrap();
        OperationServiceConfig {
            service_id,
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 60_000,
            },
            vehicle: Vehicle {
                max_speed_kmh: 72.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 40.,
            },
            dwell_ms: 100_000,
        }
    });
    (world, services, [west, east, main, side], [a, b, s, c])
}
