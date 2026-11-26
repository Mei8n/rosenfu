use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode};
use serde_json::{json, Value};

fn provenance_world() -> World {
    let mut world = World::default();
    let p = world.add_player("P".into()).unwrap();
    let c = world.add_company(p, "C".into()).unwrap();
    let a = world
        .build_alignment(
            c,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.0,
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.1,
                },
            ],
        )
        .unwrap();
    let mut draft = world.snapshot().clone();
    let key = format!("{}:{}", "a".repeat(64), "b".repeat(64));
    draft.source_records.push(SourceRecord {
        key: key.clone(),
        dataset_id: "fixture".into(),
        revision: "r1".into(),
        graph_sha256: "a".repeat(64),
        source_sha256: "b".repeat(64),
        source: "offline test fixture".into(),
        source_date: None,
        license: "ODbL-1.0".into(),
        attribution: "© OpenStreetMap contributors".into(),
        conversion_format_version: 1,
    });
    draft.alignment_provenance.push(AlignmentProvenance {
        derived_slice: None,
        alignment_id: a,
        start_point_index: 0,
        end_point_index: 1,
        source: AlignmentProvenanceInput {
            tile_fragment_id: None,
            network_path: vec![],
            source_key: key,
            method: ProvenanceMethod::OsmSegmentCopyV1,
            osm_way_id: "123".into(),
            mode: ReferenceMode::Rail,
            start_segment_index: 0,
            start_fraction: 0.1,
            end_segment_index: 0,
            end_fraction: 0.9,
        },
    });
    World::new(draft).unwrap()
}

#[test]
fn provenance_roundtrip_and_corrupt_metadata_are_strict() {
    let world = provenance_world();
    let bytes = encode(&world).unwrap();
    assert_eq!(decode(&bytes).unwrap(), world);
    let good: Value = serde_json::from_slice(&bytes).unwrap();
    for (pointer, v) in [
        ("/world/alignment_provenance/0/alignment_id", json!("999")),
        (
            "/world/alignment_provenance/0/source/source_key",
            json!("missing"),
        ),
        (
            "/world/alignment_provenance/0/source/start_fraction",
            json!(2),
        ),
        ("/world/source_records/0/graph_sha256", json!("invalid")),
        ("/world/source_records/0/attribution", json!("wrong")),
    ] {
        let mut bad = good.clone();
        *bad.pointer_mut(pointer).unwrap() = v;
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
    }
    for pointer in [
        "/world/source_records/0",
        "/world/alignment_provenance/0",
        "/world/alignment_provenance/0/source",
    ] {
        let mut bad = good.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(true));
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn versioned_network_chain_provenance_roundtrips_without_fabricated_way() {
    let world = provenance_world();
    let mut draft = world.snapshot().clone();
    draft.source_records[0].conversion_format_version = 2;
    let source = &mut draft.alignment_provenance[0].source;
    source.method = ProvenanceMethod::RailGraphRouteV1;
    source.osm_way_id.clear();
    source.mode = ReferenceMode::Rail;
    source.start_segment_index = 0;
    source.end_segment_index = 0;
    source.start_fraction = 0.0;
    source.end_fraction = 0.0;
    source.network_path = vec![RailGraphSpan {
        region: "jp".into(),
        chain_id: 7,
        from_fraction: 0.1,
        to_fraction: 0.9,
    }];
    let network_world = World::new(draft).unwrap();
    let bytes = encode(&network_world).unwrap();
    assert_eq!(decode(&bytes).unwrap(), network_world);
    let old = String::from_utf8(bytes)
        .unwrap()
        .replace("\"schema_version\": 27", "\"schema_version\": 24");
    assert!(decode(old.as_bytes()).is_err());
}

#[test]
fn unbuilt_network_plan_roundtrips_then_builds_exact_geometry_and_source() {
    let built = provenance_world();
    let mut record = built.snapshot().source_records[0].clone();
    record.conversion_format_version = 2;
    let mut source = built.snapshot().alignment_provenance[0].source.clone();
    source.method = ProvenanceMethod::RailGraphRouteV1;
    source.osm_way_id.clear();
    source.mode = ReferenceMode::Rail;
    source.start_segment_index = 0;
    source.end_segment_index = 0;
    source.start_fraction = 0.0;
    source.end_fraction = 0.0;
    source.network_path = vec![RailGraphSpan {
        region: "jp".into(),
        chain_id: 7,
        from_fraction: 0.1,
        to_fraction: 0.9,
    }];
    let points = built.snapshot().alignments[0].points.clone();
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let owner = world.add_company(player, "C".into()).unwrap();
    let plans = json!({"version":1,"plans":[{"id":1,"name":"rail"}],"parkedPaths":[{"id":2,"planId":1,"name":"rail","points":points,"redo":[],"line":points,"extension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"reference":{"name":"rail","points":points,"sourceRecord":record,"provenance":source}}],"activePathId":1,"activePlanId":1,"draftPoints":[],"redoPoints":[],"draftExtension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":false,"plannedStations":[],"draftFinished":false});
    let mut draft = world.snapshot().clone();
    draft.construction_plans = Some(serde_json::from_value(plans).unwrap());
    let saved = World::new(draft).unwrap();
    let mut restored = decode(&encode(&saved).unwrap()).unwrap();
    let reference = restored
        .snapshot()
        .construction_plans
        .as_ref()
        .unwrap()
        .parked_paths[0]
        .reference
        .clone()
        .unwrap();
    assert_eq!(reference.points, points);
    restored
        .build_alignment_from_reference(
            owner,
            reference.name,
            reference.points,
            reference.source_record,
            reference.provenance,
        )
        .unwrap();
    assert_eq!(restored.snapshot().alignments[0].points, points);
    assert_eq!(restored.snapshot().alignment_provenance[0].source, source);
    assert_eq!(decode(&encode(&restored).unwrap()).unwrap(), restored);
    let mut invalid = saved.snapshot().clone();
    invalid.construction_plans.as_mut().unwrap().parked_paths[0].line[0].latitude += 0.001;
    assert!(World::new(invalid).is_err());
}

#[test]
fn ofm_tile_provenance_survives_save_without_fake_osm_way_id() {
    let mut draft = provenance_world().snapshot().clone();
    let source = &mut draft.source_records[0];
    source.dataset_id = "ofm-tiles/20261101_080013_pt/14/14551/6465".into();
    source.revision = "20261101_080013_pt".into();
    source.conversion_format_version = 3;
    let provenance = &mut draft.alignment_provenance[0].source;
    provenance.method = ProvenanceMethod::OfmTileSliceV1;
    provenance.mode = ReferenceMode::Road;
    provenance.osm_way_id.clear();
    provenance.tile_fragment_id = Some("ofm:20261101_080013_pt:14:14551:6465:12:0".into());
    let world = World::new(draft).unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
}
