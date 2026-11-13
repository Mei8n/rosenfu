use rosenfu_core::world::*;
fn sample() -> (
    World,
    CompanyId,
    SourceRecord,
    AlignmentProvenanceInput,
    Vec<GeoPoint>,
) {
    let mut w = World::default();
    let p = w.add_player("P".into()).unwrap();
    let c = w.add_company(p, "C".into()).unwrap();
    let graph = "a".repeat(64);
    let source = "b".repeat(64);
    let key = format!("{graph}:{source}");
    let record = SourceRecord {
        key: key.clone(),
        dataset_id: "D".into(),
        revision: "R".into(),
        graph_sha256: graph,
        source_sha256: source,
        source: "Local OSM extract".into(),
        source_date: Some("2026-11-12".into()),
        license: "ODbL-1.0".into(),
        attribution: "© OpenStreetMap contributors".into(),
        conversion_format_version: 1,
    };
    let provenance = AlignmentProvenanceInput {
        source_key: key,
        method: ProvenanceMethod::OsmWaySliceV1,
        osm_way_id: "123".into(),
        mode: ReferenceMode::Rail,
        start_segment_index: 0,
        start_fraction: 0.2,
        end_segment_index: 2,
        end_fraction: 0.8,
    };
    (
        w,
        c,
        record,
        provenance,
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
}
#[test]
fn atomic_copy_deduplicates_source_and_retains_snapshots_without_bundle() {
    let (mut w, c, source, p, points) = sample();
    let a = w
        .build_alignment_from_reference(c, "A".into(), points.clone(), source.clone(), p.clone())
        .unwrap();
    w.build_alignment_from_reference(c, "B".into(), points.clone(), source.clone(), p.clone())
        .unwrap();
    assert_eq!(w.snapshot().source_records, vec![source]);
    assert_eq!(w.snapshot().alignment_provenance.len(), 2);
    assert_eq!(World::new(w.snapshot().clone()).unwrap(), w);
    let mut draft = w.snapshot().clone();
    draft.alignments.retain(|x| x.id != a);
    draft.alignment_provenance.retain(|x| x.alignment_id != a);
    w.replace(draft).unwrap();
    assert_eq!(w.snapshot().source_records.len(), 1);
    assert_eq!(w.snapshot().alignment_provenance.len(), 1);
    assert_eq!(w.snapshot().alignments[0].points, points);
}
#[test]
fn failures_leave_geometry_metadata_and_allocator_unchanged() {
    let (mut w, c, source, p, points) = sample();
    let before = w.clone();
    let mut bad = source.clone();
    bad.source = "x".repeat(1001);
    assert!(w
        .build_alignment_from_reference(c, "A".into(), points.clone(), bad, p.clone())
        .is_err());
    let mut bad = source.clone();
    bad.source_date = Some("x".repeat(65));
    assert!(w
        .build_alignment_from_reference(c, "A".into(), points.clone(), bad, p.clone())
        .is_err());
    let mut bad = source.clone();
    bad.source = "Untrusted\u{202e}control".into();
    assert!(w
        .build_alignment_from_reference(c, "A".into(), points.clone(), bad, p.clone())
        .is_err());
    let mut bad = p.clone();
    bad.start_fraction = f64::NAN;
    assert!(w
        .build_alignment_from_reference(c, "A".into(), points.clone(), source.clone(), bad)
        .is_err());
    let mut bad = p.clone();
    bad.source_key = "missing".into();
    assert!(w
        .build_alignment_from_reference(c, "A".into(), points.clone(), source.clone(), bad)
        .is_err());
    assert!(w
        .build_alignment_from_reference(c, "A".into(), vec![], source, p)
        .is_err());
    assert_eq!(w, before);
}
#[test]
fn source_conflicts_ref_changes_and_geometry_changes_are_rejected() {
    let (mut w, c, source, p, points) = sample();
    w.build_alignment_from_reference(c, "A".into(), points.clone(), source.clone(), p.clone())
        .unwrap();
    let before = w.clone();
    let mut source = source;
    source.revision = "different".into();
    assert!(w
        .build_alignment_from_reference(c, "B".into(), points, source, p)
        .is_err());
    let mut draft = w.snapshot().clone();
    draft.source_records.clear();
    assert!(w.replace(draft).is_err());
    let mut draft = w.snapshot().clone();
    draft.alignment_provenance.clear();
    assert_eq!(w.replace(draft), Err(WorldError::ProvenanceEditUnsupported));
    let mut draft = w.snapshot().clone();
    draft.alignments[0].points[1].longitude += 0.01;
    assert_eq!(w.replace(draft), Err(WorldError::ProvenanceEditUnsupported));
    let mut draft = w.snapshot().clone();
    draft.alignments.clear();
    assert!(w.replace(draft).is_err());
    let mut draft = w.snapshot().clone();
    draft
        .alignment_provenance
        .push(draft.alignment_provenance[0].clone());
    assert!(w.replace(draft).is_err());
    assert_eq!(w, before);
    let mut draft = w.snapshot().clone();
    draft.alignments[0].name = "Renamed".into();
    w.replace(draft).unwrap();
    assert_eq!(
        w.snapshot().source_records,
        before.snapshot().source_records
    );
}
#[test]
fn changed_dataset_version_is_an_additional_immutable_record_not_replacement() {
    let (mut w, c, source, p, points) = sample();
    w.build_alignment_from_reference(c, "Old".into(), points.clone(), source.clone(), p.clone())
        .unwrap();
    let mut new = source;
    new.graph_sha256 = "c".repeat(64);
    new.key = format!("{}:{}", new.graph_sha256, new.source_sha256);
    new.revision = "New".into();
    let mut new_p = p;
    new_p.source_key = new.key.clone();
    w.build_alignment_from_reference(c, "New".into(), points, new, new_p)
        .unwrap();
    assert_eq!(w.snapshot().source_records.len(), 2);
    assert_ne!(
        w.snapshot().alignment_provenance[0].source.source_key,
        w.snapshot().alignment_provenance[1].source.source_key
    );
}

#[test]
fn invalid_source_ids_hashes_versions_and_slice_metadata_are_rejected() {
    let (mut w, c, source, p, points) = sample();
    let before = w.clone();
    for field in ["hash", "key", "attribution", "version", "license"] {
        let mut bad = source.clone();
        match field {
            "hash" => bad.graph_sha256 = "A".repeat(64),
            "key" => bad.key = "wrong".into(),
            "attribution" => bad.attribution = "Unknown".into(),
            "license" => bad.license = "Unknown".into(),
            _ => bad.conversion_format_version = 2,
        };
        assert!(w
            .build_alignment_from_reference(c, "A".into(), points.clone(), bad, p.clone())
            .is_err());
    }
    for field in ["way", "fraction", "index", "point", "segment"] {
        let mut bad = p.clone();
        match field {
            "way" => bad.osm_way_id = "01".into(),
            "fraction" => bad.end_fraction = 1.1,
            "index" => bad.end_segment_index = 9_007_199_254_740_992,
            "point" => {
                bad.start_segment_index = 0;
                bad.start_fraction = 1.0;
                bad.end_segment_index = 1;
                bad.end_fraction = 0.0;
            }
            _ => bad.method = ProvenanceMethod::OsmSegmentCopyV1,
        };
        assert!(w
            .build_alignment_from_reference(c, "A".into(), points.clone(), source.clone(), bad)
            .is_err());
    }
    assert_eq!(w, before);
}

#[test]
fn source_and_provenance_count_limits_are_enforced() {
    let (mut w, c, source, p, points) = sample();
    w.build_alignment_from_reference(c, "A".into(), points, source, p)
        .unwrap();
    let before = w.clone();
    let mut draft = w.snapshot().clone();
    draft.source_records = vec![draft.source_records[0].clone(); 1025];
    assert_eq!(World::new(draft), Err(WorldError::InvalidProvenance));
    let mut draft = w.snapshot().clone();
    draft.alignment_provenance = vec![draft.alignment_provenance[0].clone(); 10001];
    assert_eq!(World::new(draft), Err(WorldError::InvalidProvenance));
    assert_eq!(w, before);
}

#[test]
fn metadata_byte_budget_rejects_distinct_valid_records_below_count_limits() {
    let (mut w, c, source, p, points) = sample();
    w.build_alignment_from_reference(c, "A".into(), points, source, p)
        .unwrap();
    let mut draft = w.snapshot().clone();
    let template = draft.source_records[0].clone();
    draft.source_records = (0..1024)
        .map(|i| {
            let mut source = template.clone();
            source.graph_sha256 = format!("{i:064x}");
            source.key = format!("{}:{}", source.graph_sha256, source.source_sha256);
            source.dataset_id = "d".repeat(200);
            source.revision = "r".repeat(200);
            source.source = "s".repeat(1000);
            source.source_date = Some("t".repeat(64));
            source
        })
        .collect();
    draft.alignment_provenance[0].source.source_key = draft.source_records[0].key.clone();
    // This proves all records are individually valid and below the byte budget.
    assert!(World::new(draft.clone()).is_ok());
    let alignment = draft.alignments[0].clone();
    let provenance = draft.alignment_provenance[0].clone();
    draft.alignments = (0..500)
        .map(|i| {
            let mut a = alignment.clone();
            a.id = AlignmentId(i + 3);
            a
        })
        .collect();
    draft.alignment_provenance = (0..500)
        .map(|i| {
            let mut p = provenance.clone();
            p.alignment_id = AlignmentId(i + 3);
            p
        })
        .collect();
    draft.next_id = 503;
    // Distinct valid IDs, source and provenance counts stay within their caps.
    assert_eq!(World::new(draft), Err(WorldError::InvalidProvenance));
}

#[test]
fn reference_geometry_copy_keeps_selected_track_type_independent_of_source_mode() {
    for kind in [TrackType::Normal, TrackType::HighSpeed, TrackType::Tram] {
        let (mut w, c, source, p, points) = sample();
        w.build_alignment_from_reference_typed(
            c,
            "Typed".into(),
            points.clone(),
            source.clone(),
            p.clone(),
            kind,
        )
        .unwrap();
        let a = &w.snapshot().alignments[0];
        assert_eq!(a.track_type, kind);
        assert_eq!(a.points, points);
        assert_eq!(w.snapshot().source_records, vec![source]);
        assert_eq!(w.snapshot().alignment_provenance[0].source, p);
    }
}

#[test]
fn extension_preserves_source_span_and_stations_at_both_ends() {
    let (mut w, c, source, provenance, points) = sample();
    let a = w
        .build_alignment_from_reference(
            c,
            "A".into(),
            points.clone(),
            source.clone(),
            provenance.clone(),
        )
        .unwrap();
    w.place_station_on_alignment(c, "S".into(), a, 0.5, 100.0, 12.0)
        .unwrap();
    let second = w
        .place_station_on_alignment(c, "Second".into(), a, 0.8, 100.0, 12.0)
        .unwrap();
    let first = w.snapshot().stations[0].id;
    let segments = vec![RouteSegment::Alignment {
        alignment_id: a,
        from: first,
        to: second,
    }];
    let stops = w
        .preview_service_path(first, segments.clone())
        .unwrap()
        .occurrences
        .into_iter()
        .map(|o| o.reference)
        .collect();
    let service = w
        .add_service_path(c, "Service".into(), first, segments, stops)
        .unwrap();
    let services = w.snapshot().service_paths.clone();
    let occurrences = w.service_path_occurrences(service).unwrap();
    let stations = w.snapshot().stations.clone();
    w.extend_alignment(
        c,
        a,
        AlignmentEndpoint::End,
        vec![
            points[1],
            GeoPoint {
                latitude: 35.0,
                longitude: 139.2,
            },
        ],
    )
    .unwrap();
    w.extend_alignment(
        c,
        a,
        AlignmentEndpoint::Start,
        vec![
            points[0],
            GeoPoint {
                latitude: 35.0,
                longitude: 138.9,
            },
        ],
    )
    .unwrap();
    let p = &w.snapshot().alignment_provenance[0];
    assert_eq!((p.start_point_index, p.end_point_index), (1, 2));
    assert_eq!(p.source, provenance);
    assert_eq!(w.snapshot().source_records, vec![source]);
    assert_eq!(w.snapshot().stations, stations);
    assert_eq!(w.snapshot().service_paths, services);
    assert_eq!(w.service_path_occurrences(service).unwrap(), occurrences);
    assert_eq!(&w.snapshot().alignments[0].points[1..=2], points.as_slice());
    let before = w.snapshot().clone();
    let mut tampered = before.clone();
    tampered.alignment_provenance[0].start_point_index = 0;
    assert_eq!(
        w.replace(tampered),
        Err(WorldError::ProvenanceEditUnsupported)
    );
    assert_eq!(w.snapshot(), &before);
    for (start, end) in [(1, 1), (2, 1), (0, 4)] {
        let mut bad = before.clone();
        bad.alignment_provenance[0].start_point_index = start;
        bad.alignment_provenance[0].end_point_index = end;
        assert!(World::new(bad).is_err());
    }
}

#[test]
fn multiple_reference_extensions_keep_separate_sources_and_orientation() {
    let (mut w, c, source, p, points) = sample();
    let id = w
        .build_alignment_from_reference(c, "A".into(), points.clone(), source.clone(), p.clone())
        .unwrap();
    let mut other = source.clone();
    other.source_sha256 = "c".repeat(64);
    other.key = format!("{}:{}", other.graph_sha256, other.source_sha256);
    let mut provenance = p.clone();
    provenance.source_key = other.key.clone();
    provenance.osm_way_id = "456".into();
    w.extend_alignment_from_reference(
        c,
        id,
        AlignmentEndpoint::End,
        vec![
            points[1],
            GeoPoint {
                latitude: 35.0,
                longitude: 139.2,
            },
        ],
        other.clone(),
        provenance.clone(),
    )
    .unwrap();
    w.extend_alignment_from_reference(
        c,
        id,
        AlignmentEndpoint::Start,
        vec![
            points[0],
            GeoPoint {
                latitude: 35.0,
                longitude: 138.9,
            },
        ],
        other.clone(),
        provenance.clone(),
    )
    .unwrap();
    let data = w.snapshot();
    assert_eq!(data.source_records, vec![source, other]);
    assert_eq!(
        data.alignment_provenance
            .iter()
            .map(|p| (p.start_point_index, p.end_point_index))
            .collect::<Vec<_>>(),
        vec![(1, 2), (2, 3), (0, 1)]
    );
    assert_eq!(
        data.alignment_provenance[2].source.start_segment_index,
        provenance.end_segment_index
    );
    assert_eq!(
        data.alignment_provenance[2].source.end_fraction,
        provenance.start_fraction
    );
    let mut overlapping = data.clone();
    overlapping.alignment_provenance[1].start_point_index = 1;
    assert!(World::new(overlapping).is_err());
    let before = w.snapshot().clone();
    let mut bad = w.snapshot().source_records[0].clone();
    bad.attribution = "removed".into();
    assert!(w
        .extend_alignment_from_reference(
            c,
            id,
            AlignmentEndpoint::End,
            vec![
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.2
                },
                GeoPoint {
                    latitude: 35.0,
                    longitude: 139.3
                }
            ],
            bad,
            p
        )
        .is_err());
    assert_eq!(w.snapshot(), &before);
}
