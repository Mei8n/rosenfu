#[allow(dead_code, unused_imports)]
#[path = "../../../src-tauri/src/session.rs"]
mod session;
#[test]
fn qa_is_explicit_persistent_and_undoable() {
    let mut editor = session::WorldEditor::new();
    let before = editor.snapshot();
    assert!(editor.enable_road_qa_fixture(false).is_err());
    assert_eq!(editor.snapshot(), before);
    let graph = editor.enable_road_qa_fixture(true).unwrap();
    assert_eq!(graph.road_transport.edges.len(), 4);
    let world = rosenfu_core::world::World::new(graph.clone()).unwrap();
    let restored = rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap()).unwrap();
    assert_eq!(restored.snapshot(), &graph);
    let undone = editor.undo_world_edit(graph.clone()).unwrap();
    assert_eq!(undone.road_transport, before.road_transport);
    let redone = editor.redo_world_edit(undone).unwrap();
    assert_eq!(redone.road_transport, graph.road_transport);
    assert!(editor.enable_road_qa_fixture(true).is_err());
}
#[test]
fn normal_ipc_cannot_forge_access_or_graph() {
    let mut editor = session::WorldEditor::new();
    let world = editor.enable_road_qa_fixture(true).unwrap();
    let mut changed = world.road_transport.clone();
    changed.edges[0].source = "pretend-live".into();
    assert!(editor
        .replace_road_transport(world.road_transport.clone(), changed)
        .is_err());
    assert_eq!(editor.snapshot(), world);
}
#[test]
fn road_tram_native_lifecycle_is_atomic_and_undoable() {
    let mut editor = session::WorldEditor::new();
    let initial = editor.enable_road_qa_fixture(true).unwrap();
    let invalid = editor.edit_road_tram(session::RoadTramEdit::Build {
        from: "qa:0".into(),
        to: "missing".into(),
        name: "QA".into(),
        shared: true,
    });
    assert!(invalid.is_err());
    assert_eq!(editor.snapshot(), initial);
    let built = editor
        .edit_road_tram(session::RoadTramEdit::Build {
            from: "qa:0".into(),
            to: "qa:1".into(),
            name: "QA軌道".into(),
            shared: true,
        })
        .unwrap();
    let alignment_id = built.road_transport.tram_sections[0].alignment_id;
    assert_eq!(built.alignments.len(), 1);
    let dedicated = editor
        .edit_road_tram(session::RoadTramEdit::ChangePlacement {
            alignment_id,
            from: String::new(),
            to: String::new(),
            shared: false,
        })
        .unwrap();
    assert!(matches!(
        dedicated.road_transport.tram_sections[0].placement,
        rosenfu_core::road_transport::TramPlacement::Dedicated
    ));
    let world = rosenfu_core::world::World::new(dedicated.clone()).unwrap();
    assert_eq!(
        rosenfu_storage::decode(&rosenfu_storage::encode(&world).unwrap())
            .unwrap()
            .snapshot(),
        &dedicated
    );
    let removed = editor
        .edit_road_tram(session::RoadTramEdit::Remove { alignment_id })
        .unwrap();
    assert!(removed.alignments.is_empty() && removed.road_transport.tram_sections.is_empty());
    assert_eq!(editor.undo_world_edit(removed).unwrap(), dedicated);
}

#[test]
fn unknown_native_pilot_and_altered_save_never_authorize_graph() {
    let mut editor = session::WorldEditor::new();
    let before = editor.snapshot();
    assert!(editor
        .install_native_road_pilot("missing-corridor")
        .is_err());
    assert!(editor
        .install_native_road_pilot("../../local-data.json")
        .is_err());
    assert_eq!(editor.snapshot(), before);
    let qa = editor.enable_road_qa_fixture(true).unwrap();
    assert!(editor
        .install_native_road_pilot("missing-corridor")
        .is_err());
    assert_eq!(editor.snapshot(), qa);

    let world = rosenfu_core::world::World::new(qa.clone()).unwrap();
    let encoded = rosenfu_storage::encode(&world).unwrap();
    let encoded = String::from_utf8(encoded).unwrap();
    assert!(encoded.contains("synthetic-qa-not-real-roads"));
    for fake in ["native-pilot:unreviewed", "independent-fake-provider"] {
        let tampered = encoded.replace("synthetic-qa-not-real-roads", fake);
        assert!(rosenfu_storage::decode(tampered.as_bytes()).is_err());
    }
}

#[test]
fn ordinary_authored_tram_intersection_build_operate_and_reopen() {
    use base64::Engine;
    use rosenfu_core::ofm_evidence::{AuthoredTramGuide, BusVertex};
    let mut editor = session::WorldEditor::new();
    editor.rename_company("合成道路交差テスト".into()).unwrap();
    let original = editor.snapshot();
    let undo_depth = editor.world_edit_history().undo_depth;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../rosenfu-core/tests/fixtures/ofm_authored_tram_synthetic.json"
    ))
    .unwrap();
    let tile_key = fixture["tileKey"].as_str().unwrap();
    let digest = fixture["tileRawSha256"].as_str().unwrap();
    let guide = AuthoredTramGuide {
        adjacent_end: None,
        start_fraction_q: None,
        end_fraction_q: None,
        tile_key: tile_key.into(),
        tile_raw_sha256: digest.into(),
        start: BusVertex {
            feature: 0,
            line: 0,
            vertex: 0,
        },
        end: BusVertex {
            feature: 1,
            line: 0,
            vertex: 1,
        },
    };
    let cached = || {
        let bytes: Vec<u8> = serde_json::from_value(fixture["raw"].clone()).unwrap();
        (
            session::OfmRawInput {
                sha256: digest.into(),
                byte_length: bytes.len(),
                base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
            session::OfmBindingInput {
                key: tile_key.into(),
                url: tile_key.into(),
                raw_sha256: digest.into(),
                revision: fixture["revision"].as_str().unwrap().into(),
                z: 14,
                x: 14552,
                y: 6451,
                decoder_rule: "ofm_transportation_z14_v1".into(),
                license: "ODbL-1.0".into(),
                attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
            },
        )
    };
    let token = editor.ofm_world_token().unwrap();
    assert!(editor
        .preview_authored_tram_road_guide("bad-token", &guide, Some(cached()))
        .is_err());
    assert!(editor
        .preview_authored_tram_road_guide(&token, &guide, None)
        .is_err());
    let points = editor
        .preview_authored_tram_road_guide(&token, &guide, Some(cached()))
        .unwrap();
    assert_eq!(points.len(), 3);
    let reverse = AuthoredTramGuide {
        start: guide.end,
        end: guide.start,
        ..guide
    };
    let opposite = editor
        .preview_authored_tram_road_guide(&token, &reverse, Some(cached()))
        .unwrap();
    assert!(points.iter().rev().eq(&opposite));
    assert_eq!(editor.snapshot(), original);
    assert_eq!(editor.ofm_world_token().unwrap(), token);
    assert_eq!(editor.world_edit_history().undo_depth, undo_depth);

    let alignment = |levels| {
        serde_json::from_value::<session::PlannedAlignment>(serde_json::json!({
            "name": "編集した架空軌道", "trackType": "tram", "points": points,
            "anchorIndices": [0, 2], "anchorLevels": levels, "stations": []
        }))
        .unwrap()
    };
    // The road evidence is horizontal only: game height/gradient still runs
    // through the normal construction gate, not a measured real rail grade.
    assert!(editor
        .build_construction_plan(vec![alignment(["ground", "elevated"])], vec![])
        .is_err());
    assert_eq!(editor.snapshot(), original);
    let built = editor
        .build_construction_plan(vec![alignment(["ground", "ground"])], vec![])
        .unwrap();
    assert_eq!(
        built.alignments[0].track_type,
        rosenfu_core::world::TrackType::Tram
    );
    assert!(built.road_path_provenance.is_empty() && built.road_evidence_descriptors.is_empty());
    assert!(built.road_transport.tram_sections.is_empty());
    assert!(built.road_transport.edges.is_empty());
    // No shared-road debug graph, pilot, or imported rail: the player
    // provisions ordinary ground platforms and a route on the built track.
    use rosenfu_core::operation::{Timetable, TrainState, Vehicle};
    use rosenfu_core::world::{RouteSegment, World};
    let id = built.alignments[0].id;
    let stops_world = editor
        .place_station_on_alignment("南西停留場".into(), id, 0.15, 20., 8.)
        .unwrap();
    let from = stops_world.stations[0].id;
    let stops_world = editor
        .place_station_on_alignment("北停留場".into(), id, 0.88, 20., 8.)
        .unwrap();
    let to = stops_world.stations[1].id;
    let path = vec![RouteSegment::Alignment {
        alignment_id: id,
        from,
        to,
    }];
    let route_world = editor
        .save_planned_route(None, "交差点線".into(), from, path.clone(), vec![])
        .unwrap();
    let route = route_world.routes[0].id;
    let stops = editor
        .preview_service_path(from, path.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|occ| occ.reference)
        .collect::<Vec<_>>();
    assert_eq!(stops.len(), 2);
    let service_world = editor
        .create_service_path_with_source_routes("各停".into(), from, path, stops, Some(route), None)
        .unwrap();
    let service = service_world.services[0].id;
    let editable = editor.operation_snapshot().timetable_edit_token.clone();
    let configured = editor
        .configure_operation_service(
            service,
            Timetable {
                first_departure_ms: 0,
                last_departure_ms: 0,
                headway_ms: 60_000,
            },
            Vehicle {
                max_speed_kmh: 95.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 12.,
            },
            15_000,
            &editable,
        )
        .unwrap();
    assert!(configured.paused && configured.trains.is_empty());
    assert_eq!(configured.services.len(), 1);
    assert!(configured.bus_services.is_empty());
    let config = &configured.services[0];
    let spec = World::new(service_world.clone())
        .unwrap()
        .compile_operation(config)
        .unwrap();
    assert_eq!(spec.legs.len(), 2);
    assert_eq!(spec.legs[0].from, from.0.to_string());
    assert_eq!(spec.legs[0].to, to.0.to_string());
    assert_eq!(spec.legs[1].from, to.0.to_string());
    assert_eq!(spec.legs[1].to, from.0.to_string());
    assert!(spec.legs.iter().all(|leg| !leg.sections.is_empty()));
    assert!(spec
        .legs
        .iter()
        .flat_map(|leg| &leg.sections)
        .all(|section| section.speed_limit_kmh <= 50.));

    editor.set_operation_clock(false, 1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let departed = editor.advance_operation().unwrap();
    assert!(departed.now_ms > configured.now_ms);
    assert!(!departed.paused && !departed.invalidated);
    assert_eq!(departed.trains.len(), 1);
    let train = &departed.trains[0];
    assert_eq!(train.key.service_id, service.0.to_string());
    assert_eq!(train.key.scheduled_ms, 0);
    assert_eq!(train.leg, 0);
    let TrainState::Moving {
        entered_ms,
        arrival_ms,
        release_ms,
    } = train.state
    else {
        panic!("ordinary tram did not depart");
    };
    assert_eq!(entered_ms, 0);
    assert!(arrival_ms > departed.now_ms);

    // Autonomously ticked via the ordinary session clock; pause before disk I/O
    // to compare the exact in-flight clock and train after session reload.
    let before_save = editor.set_operation_clock(true, 1).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("player-authored-tram.json");
    editor.save(&file).unwrap();
    let mut opened_editor = session::WorldEditor::new();
    assert_eq!(opened_editor.load(&file).unwrap(), service_world);
    let reopened = opened_editor.operation_snapshot();
    assert!(reopened.paused && !reopened.invalidated);
    assert_eq!(reopened.now_ms, before_save.now_ms);
    assert_eq!(reopened.trains, before_save.trains);
    assert_eq!(reopened.services, before_save.services);

    // Finish the exact saved in-flight trip with the deterministic shared
    // scheduler, instead of waiting real seconds or injecting a train.
    let mut world = rosenfu_storage::load_world(&file).unwrap();
    let runtime = &mut world.operation_mut().unwrap().runtime;
    runtime.set_paused(false);
    let arrived = runtime.advance_to(arrival_ms, 128).unwrap();
    assert!(arrived.reached_target);
    assert_eq!(arrived.arrivals.len(), 1);
    assert_eq!(arrived.arrivals[0].station_id, to.0.to_string());
    assert_eq!(arrived.arrivals[0].train, train.key);
    assert_eq!(arrived.arrivals[0].delay_ms, 0);
    let cleared = runtime.advance_to(release_ms, 128).unwrap();
    assert!(cleared.reached_target);
    assert!(cleared.arrivals.is_empty());
    assert_eq!(
        runtime.trains().map(|train| train.leg).collect::<Vec<_>>(),
        vec![1]
    );
    let returned = runtime.advance_to(release_ms + 120_000, 128).unwrap();
    assert!(returned.reached_target);
    assert_eq!(returned.arrivals.len(), 1);
    assert_eq!(returned.arrivals[0].station_id, from.0.to_string());
    assert_eq!(returned.arrivals[0].train, train.key);
    assert_eq!(runtime.trains().count(), 0);
}

#[test]
fn ordinary_shared_seam_survives_stock_world_editor_save_and_reopen() {
    use base64::Engine;
    use rosenfu_core::ofm_evidence::{
        BusPathBinding, BusTileProof, BusVertex, BUS_RECTANGLE_RULE, BUS_SHARED_SEAM_RULE,
        DECODER_RULE,
    };
    use session::{OfmBindingInput, OfmRawInput, WorldEditor};

    let mut editor = WorldEditor::new();
    editor.rename_company("バス会社".into()).unwrap();
    // Same synthetic buffered feature as the native v3 proof test. The
    // two cuts differ; no installed catalog, fixture import, or alternate
    // storage reader participates in this normal authoring path.
    let input = [
        "Gk8KDnRyYW5zcG9ydGF0aW9ueAIogCASGAgVEgQAAAEBGAIiDAmgQZAcEicT1wSrAhoFY2xhc3MaBm9uZXdheSILCglzZWNvbmRhcnkiAigB",
        "GlIKDnRyYW5zcG9ydGF0aW9ueAIogCASGwgVEgQAAAEBGAIiDwnoAvQcGscBYycTyRalCxoFY2xhc3MaBm9uZXdheSILCglzZWNvbmRhcnkiAigB",
    ];
    let cached = || {
        input
            .into_iter()
            .enumerate()
            .map(|(i, raw)| {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(raw)
                    .unwrap();
                let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
                let x = 14552 + i as u32;
                let key =
                    format!("https://tiles.openfreemap.org/planet/synthetic_stock/14/{x}/6451.pbf");
                (
                    OfmRawInput {
                        sha256: sha.clone(),
                        byte_length: bytes.len(),
                        base64: raw.into(),
                    },
                    OfmBindingInput {
                        key: key.clone(),
                        raw_sha256: sha,
                        url: key,
                        revision: "synthetic_stock".into(),
                        z: 14,
                        x,
                        y: 6451,
                        decoder_rule: DECODER_RULE.into(),
                        license: "ODbL-1.0".into(),
                        attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                    },
                )
            })
            .collect::<Vec<_>>()
    };
    let inputs = cached();
    let west = &inputs[0].1;
    let east = &inputs[1].1;
    let mut proof = BusPathBinding {
        rule: BUS_RECTANGLE_RULE.into(),
        same_atom_stops: None,
        fractional_terminals: None,
        grade_policy: None,
        tile_key: east.key.clone(),
        tile_raw_sha256: east.raw_sha256.clone(),
        end_tile_key: Some(west.key.clone()),
        pattern: None,
        interior_stops: vec![],
        via: vec![],
        additional_tiles: vec![BusTileProof {
            tile_key: west.key.clone(),
            tile_raw_sha256: west.raw_sha256.clone(),
        }],
        respect_oneway: true,
        start: BusVertex {
            feature: 0,
            line: 0,
            vertex: 1,
        },
        end: BusVertex {
            feature: 0,
            line: 0,
            vertex: 2,
        },
    };
    let route = "stock-seam";
    let name = "境界横断";
    let before = editor.snapshot();
    let token = editor.ofm_world_token().unwrap();
    let refused = editor
        .preview_gameplay_ofm_bus(&token, route.into(), name.into(), proof.clone(), cached())
        .unwrap();
    assert_eq!(refused.failed_leg_index, Some(0));
    assert!(refused.points.is_empty());
    proof.rule = BUS_SHARED_SEAM_RULE.into();
    let exact = editor
        .preview_gameplay_ofm_bus(&token, route.into(), name.into(), proof.clone(), cached())
        .unwrap();
    assert_eq!(exact.failed_leg_index, None);
    assert_eq!(exact.points.len(), 4);
    let mut altered = cached();
    altered[0].0.base64.replace_range(5..6, "Z");
    assert!(editor
        .put_gameplay_ofm_bus(&token, route.into(), name.into(), proof.clone(), altered)
        .is_err());
    assert_eq!(editor.snapshot(), before);
    assert_eq!(editor.ofm_world_token().unwrap(), token);
    let created = editor
        .put_gameplay_ofm_bus(&token, route.into(), name.into(), proof, cached())
        .unwrap();
    assert_eq!(created.road_evidence_descriptors.len(), 2);
    assert!(created.ofm_installed_seam_bus_routes.is_empty());
    assert_eq!(created.road_transport.bus_routes[0].edges.len(), 3);
    for sha in [&west.raw_sha256, &east.raw_sha256] {
        assert!(created
            .road_transport
            .edges
            .iter()
            .any(|edge| &edge.revision == sha));
    }
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("stock-seam.rosenfu");
    editor.save(&file).unwrap();
    let mut fresh = WorldEditor::new();
    fresh.load(&file).unwrap();
    assert_eq!(fresh.snapshot(), created);
    assert_eq!(
        fresh
            .preview_gameplay_ofm_bus(
                &fresh.ofm_world_token().unwrap(),
                route.into(),
                name.into(),
                created.ofm_gameplay_bus_routes[route].clone(),
                vec![],
            )
            .unwrap(),
        exact
    );
}
