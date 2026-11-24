use base64::Engine;
use rosenfu_core::{
    ofm_evidence::{
        find_gameplay_bus_path, raw_sha256, AuthoredTramGuide, BusVertex, EvidenceStore,
        RawEvidence, TileBinding, DECODER_RULE,
    },
    world::{CoverageBounds, GeoPoint, RoadSnapshot, World, WorldDraft},
};
use rosenfu_storage::{decode, encode};

fn fixture() -> (RawEvidence, TileBinding) {
    // Same four-vertex minor road used by the ordinary native
    // Bus stop regression, wholly inside its owning z14 tile.
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(concat!(
            "Gj4KDnRyYW5zcG9ydGF0aW9ueAIogCASFxICAAAYAiIPCcgByAEa",
            "2AQA2AQA2AQAGgVjbGFzcyIHCgVtaW5vcg=="
        ))
        .unwrap();
    let sha256 = raw_sha256(&bytes);
    let tile = TileBinding {
        canonical_url: "https://tiles.openfreemap.org/planet/fixture/14/14552/6451.pbf".into(),
        revision: "fixture".into(),
        z: 14,
        x: 14552,
        y: 6451,
        raw_sha256: sha256.clone(),
        decoder_rule: DECODER_RULE.into(),
    };
    (RawEvidence { sha256, bytes }, tile)
}

fn copy_raw(raw: &RawEvidence) -> RawEvidence {
    RawEvidence {
        sha256: raw.sha256.clone(),
        bytes: raw.bytes.clone(),
    }
}

#[test]
fn ordinary_bus_retains_one_tile_for_tram_guide_and_observed_crossing_after_reopen() {
    let (raw, tile) = fixture();
    let store = EvidenceStore::from_parts(vec![copy_raw(&raw)], vec![tile.clone()]).unwrap();
    let vertex = |vertex| BusVertex {
        feature: 0,
        line: 0,
        vertex,
    };
    let bus = find_gameplay_bus_path(&store, &tile.canonical_url, vertex(0), vertex(2), false)
        .unwrap()
        .unwrap();
    let [ax, ay] = bus.points[0];
    let [bx, by] = bus.points[1];
    let x = (ax + bx) / 2.;
    let y = (ay + by) / 2.;
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let company = world.add_company(player, "C".into()).unwrap();
    let rails = world
        .build_alignment(
            company,
            "cross".into(),
            vec![
                GeoPoint {
                    longitude: x - (by - ay),
                    latitude: y + (bx - ax),
                },
                GeoPoint {
                    longitude: x + (by - ay),
                    latitude: y - (bx - ax),
                },
            ],
        )
        .unwrap();
    assert!(world.road_assessment(rails).unwrap().crossings.is_empty());
    let world = world
        .candidate_with_gameplay_ofm_bus(
            company,
            "ordinary".into(),
            "local bus".into(),
            bus.binding.clone(),
            vec![(raw, tile.clone())],
        )
        .unwrap();
    let bytes = encode(&world).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(data["schema_version"], 27);
    assert_eq!(
        data["road_evidence"]["raw_blobs"].as_array().unwrap().len(),
        1
    );
    assert!(world.snapshot().road_snapshots.is_empty());
    let guide = AuthoredTramGuide {
        tile_key: tile.canonical_url.clone(),
        tile_raw_sha256: tile.raw_sha256.clone(),
        start: vertex(0),
        end: vertex(2),
        start_fraction_q: None,
        end_fraction_q: None,
    };
    for source in [&world, &decode(&bytes).unwrap()] {
        let crossing = source.road_assessment(rails).unwrap();
        assert!(
            !crossing.roads_ready,
            "one observed tile cannot prove complete coverage"
        );
        assert_eq!(crossing.crossings.len(), 1);
        let crossing = &crossing.crossings[0];
        assert_eq!(crossing.road_id, bus.spans[0].fragment_id);
        assert!((crossing.point.longitude - x).abs() < 1e-10);
        assert!((crossing.point.latitude - y).abs() < 1e-10);
        let tram = source
            .snapshot()
            .clone()
            .authored_tram_road_guide(&guide, vec![])
            .unwrap()
            .unwrap();
        assert_eq!(tram, bus.points);
        assert_eq!(
            encode(source).unwrap(),
            bytes,
            "advisory read cannot alter saved data"
        );
    }

    // An explicit snapshot for the same canonical URL wins even if its old
    // capture asserted different bytes. Never blend it with the retained raw.
    let mut saved = world.clone();
    saved
        .add_road_snapshot(RoadSnapshot {
            revision: tile.revision.clone(),
            source: tile.canonical_url.clone(),
            source_url: tile.canonical_url.clone(),
            source_sha256: "f".repeat(64),
            attribution: "fixture".into(),
            license: "ODbL-1.0".into(),
            coverage_verified: false,
            road_coverage: vec![CoverageBounds {
                west: x - 0.001,
                south: y - 0.001,
                east: x + 0.001,
                north: y + 0.001,
            }],
            roads: vec![],
        })
        .unwrap();
    assert!(saved.road_assessment(rails).unwrap().crossings.is_empty());
}

#[test]
fn an_observed_service_way_can_be_routed_but_is_not_a_raw_fallback_crossing() {
    let (mut raw, mut tile) = fixture();
    // Widen the dictionary string (wire lengths 62, 7 and 5 are the fixture's
    // whole layer, value, and old class). Geometry stays byte-identical.
    let class = raw.bytes.len() - 5;
    assert_eq!(&raw.bytes[class..], b"minor");
    raw.bytes[1] += 2;
    raw.bytes[class - 3] += 2;
    raw.bytes[class - 1] += 2;
    raw.bytes.splice(class.., b"service".iter().copied());
    raw.sha256 = raw_sha256(&raw.bytes);
    tile.raw_sha256 = raw.sha256.clone();
    let store = EvidenceStore::from_parts(vec![raw], vec![tile.clone()]).unwrap();
    let vertex = |vertex| BusVertex {
        feature: 0,
        line: 0,
        vertex,
    };
    let path = find_gameplay_bus_path(&store, &tile.canonical_url, vertex(0), vertex(2), false)
        .unwrap()
        .unwrap();
    let [ax, ay] = path.points[0];
    let [bx, by] = path.points[1];
    let x = (ax + bx) / 2.;
    let y = (ay + by) / 2.;
    let mut world = World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap();
    let player = world.add_player("P".into()).unwrap();
    let company = world.add_company(player, "C".into()).unwrap();
    let line = world
        .build_alignment(
            company,
            "cross service".into(),
            vec![
                GeoPoint {
                    longitude: x - (by - ay),
                    latitude: y + (bx - ax),
                },
                GeoPoint {
                    longitude: x + (by - ay),
                    latitude: y - (bx - ax),
                },
            ],
        )
        .unwrap();
    let assessment = world.road_assessment(line).unwrap();
    assert!(!assessment.roads_ready);
    assert!(assessment.crossings.is_empty());
}

#[test]
fn buffered_adjacent_tiles_report_only_interior_crossings_in_warm_and_cold_scopes() {
    let (_, template) = fixture();
    // The same east-west road buffered 192 extent units on both sides of
    // the exact shared tile edge, at y=1280 in both tiles. Distinct raw tile
    // fragments are not shared physical IDs.
    let make_tile = |x, base64| {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64)
            .unwrap();
        let sha256 = raw_sha256(&bytes);
        let mut binding = template.clone();
        binding.x = x;
        binding.canonical_url =
            format!("https://tiles.openfreemap.org/planet/fixture/14/{x}/6451.pbf");
        binding.raw_sha256 = sha256.clone();
        (RawEvidence { bytes, sha256 }, binding)
    };
    let (a, ab) = make_tile(
        14552,
        "GjgKDnRyYW5zcG9ydGF0aW9ueAIogCASERICAAAYAiIJCYA9gBQKgAYAGgVjbGFzcyIHCgVtaW5vcg==",
    );
    let (b, bb) = make_tile(
        14553,
        "GjgKDnRyYW5zcG9ydGF0aW9ueAIogCASERICAAAYAiIJCf8CgBQKgAYAGgVjbGFzcyIHCgVtaW5vcg==",
    );

    let warm_raw = vec![copy_raw(&a), copy_raw(&b)];
    let warm = EvidenceStore::from_parts(warm_raw, vec![ab.clone(), bb.clone()]).unwrap();
    let cold = EvidenceStore::from_archive_parts(vec![a, b], vec![ab, bb]).unwrap();
    for store in [warm, cold] {
        let draft = WorldDraft::default().with_road_evidence(store).unwrap();
        let mut world = World::new(draft).unwrap();
        let player = world.add_player("P".into()).unwrap();
        let company = world.add_company(player, "C".into()).unwrap();
        let boundary_lon = 14553. / 16384. * 360. - 180.;
        let latitude = (std::f64::consts::PI * (1. - 2. * (6451. + 1280. / 4096.) / 16384.))
            .sinh()
            .atan()
            .to_degrees();

        for (i, offset) in [-0.0002, 0., 0.0002].into_iter().enumerate() {
            let longitude = boundary_lon + offset;
            let alignment = world
                .build_alignment(
                    company,
                    format!("rail {i}"),
                    vec![
                        GeoPoint {
                            longitude,
                            latitude: latitude - 0.0002,
                        },
                        GeoPoint {
                            longitude,
                            latitude: latitude + 0.0002,
                        },
                    ],
                )
                .unwrap();
            let assessment = world.road_assessment(alignment).unwrap();
            assert!(!assessment.roads_ready);
            if offset == 0. {
                assert!(assessment.crossings.is_empty());
            } else {
                assert_eq!(assessment.crossings.len(), 1);
                assert!((assessment.crossings[0].point.longitude - longitude).abs() < 1e-10);
            }
        }
    }
}

#[test]
fn distant_alignments_and_retained_cells_do_not_erase_a_local_crossing() {
    let (raw, tile) = fixture();
    let bindings: Vec<_> = (0..17)
        .map(|i| {
            let mut binding = tile.clone();
            binding.x += i * 2;
            binding.canonical_url = format!(
                "https://tiles.openfreemap.org/planet/fixture/14/{}/6451.pbf",
                binding.x
            );
            binding
        })
        .collect();
    let store = EvidenceStore::from_archive_parts(vec![raw], bindings).unwrap();
    let vertex = |vertex| BusVertex {
        feature: 0,
        line: 0,
        vertex,
    };
    let local_store = store
        .scope(std::slice::from_ref(&tile.canonical_url))
        .unwrap();
    let bus = find_gameplay_bus_path(
        &local_store,
        &tile.canonical_url,
        vertex(0),
        vertex(2),
        false,
    )
    .unwrap()
    .unwrap();
    let [ax, ay] = bus.points[0];
    let [bx, by] = bus.points[1];
    let longitude = (ax + bx) / 2.;
    let latitude = (ay + by) / 2.;
    let draft = WorldDraft::default().with_road_evidence(store).unwrap();
    let mut world = World::new(draft).unwrap();
    let player = world.add_player("P".into()).unwrap();
    let company = world.add_company(player, "C".into()).unwrap();
    let add_crossing = |world: &mut World, index, x| {
        world
            .build_alignment(
                company,
                format!("rail {index}"),
                vec![
                    GeoPoint {
                        longitude: x - (by - ay),
                        latitude: latitude + (bx - ax),
                    },
                    GeoPoint {
                        longitude: x + (by - ay),
                        latitude: latitude - (bx - ax),
                    },
                ],
            )
            .unwrap()
    };
    let local = add_crossing(&mut world, 0, longitude);
    let expected = world.road_assessment(local).unwrap();
    assert_eq!(expected.crossings.len(), 1);
    assert!(!expected.roads_ready);

    // Seventeen disjoint alignments touch seventeen distinct retained cells.
    // None is in the exact local query for the original alignment.
    for i in 1..17 {
        let another_longitude = longitude + f64::from(i * 2) / 16384. * 360.;
        add_crossing(&mut world, i, another_longitude);
    }
    assert_eq!(world.road_assessment(local).unwrap(), expected);

    // Beyond the committed-snapshot broad phase threshold. Distant lines do
    // not add local raw bytes, and cannot evict this result on recomputation.
    for i in 17..65 {
        add_crossing(&mut world, i, longitude - f64::from(i));
    }
    assert_eq!(world.road_assessment(local).unwrap(), expected);
}
