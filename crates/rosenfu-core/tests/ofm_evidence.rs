use rosenfu_core::ofm_evidence::*;
fn v(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while n >= 128 {
        out.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    out.push(n as u8);
    out
}
fn field(tag: u64, data: &[u8]) -> Vec<u8> {
    [v(tag * 8 + 2), v(data.len() as u64), data.to_vec()].concat()
}
fn scalar(tag: u64, n: u64) -> Vec<u8> {
    [v(tag * 8), v(n)].concat()
}
fn packed(ns: &[u64]) -> Vec<u8> {
    ns.iter().flat_map(|n| v(*n)).collect()
}
fn feature(tags: &[u64], geometry: &[u64]) -> Vec<u8> {
    [
        field(2, &packed(tags)),
        scalar(3, 2),
        field(4, &packed(geometry)),
    ]
    .concat()
}
fn layer(features: &[Vec<u8>], keys: &[&str], values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = [field(1, b"transportation"), scalar(15, 2), scalar(5, 4096)].concat();
    for f in features {
        out.extend(field(2, f));
    }
    for k in keys {
        out.extend(field(3, k.as_bytes()));
    }
    for x in values {
        out.extend(field(4, x));
    }
    out
}
fn tile(geometry: &[u64], extra: Option<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut keys = vec!["class"];
    let mut values = vec![field(1, b"minor")];
    let mut tags = vec![0, 0];
    if let Some((k, x)) = extra {
        keys.push(k);
        values.push(x);
        tags.extend([1, 1]);
    }
    field(3, &layer(&[feature(&tags, geometry)], &keys, &values))
}
fn binding(raw: &[u8]) -> TileBinding {
    TileBinding {
        canonical_url: "https://tiles.openfreemap.org/planet/20261101_080013_pt/14/14553/6451.pbf"
            .into(),
        revision: "20261101_080013_pt".into(),
        z: 14,
        x: 14553,
        y: 6451,
        raw_sha256: raw_sha256(raw),
        decoder_rule: DECODER_RULE.into(),
    }
}
const LINE: &[u64] = &[9, 0, 0, 18, 20, 0, 0, 20];
#[test]
fn golden_multiline_and_typed_integer_context() {
    let raw = tile(
        &[9, 0, 0, 18, 20, 0, 0, 20, 9, 1, 1, 10, 2, 2],
        Some(("oneway", scalar(4, u64::MAX))),
    );
    let out = decode_tile(&binding(&raw), &raw).unwrap();
    assert_eq!(out.lines.len(), 2);
    assert_eq!(out.lines[0].coordinates, vec![[0, 0], [10, 0], [10, 10]]);
    assert_eq!(out.lines[1].coordinates, vec![[9, 9], [10, 10]]);
    assert_eq!(out.lines[1].line_index, 1);
    assert_eq!(out.lines[0].attributes["oneway"], RawValue::Int(-1));
    assert_eq!(
        out.canonical_sha256,
        "87e09bece7e57e26b2259b755507243ff50f709e26950a49129bf7c590c980db"
    );
}
#[test]
fn binding_hash_and_rule_cannot_be_substituted() {
    let raw = tile(LINE, None);
    let b = binding(&raw);
    for change in 0..6 {
        let mut x = b.clone();
        match change {
            0 => x.revision = "other".into(),
            1 => x.x += 1,
            2 => x.z = 13,
            3 => x.decoder_rule = "future".into(),
            4 => x.canonical_url.push_str("?x=1"),
            _ => x.raw_sha256 = "0".repeat(64),
        }
        assert!(decode_tile(&x, &raw).is_err());
    }
    let mut changed = raw.clone();
    changed.push(0);
    assert_eq!(decode_tile(&b, &changed), Err(Error::Hash));
}
#[test]
fn same_raw_different_tile_is_separately_bound() {
    let raw = tile(LINE, None);
    let a = binding(&raw);
    let mut b = a.clone();
    b.x += 1;
    b.canonical_url = b.canonical_url.replace("14553", "14554");
    assert_ne!(
        decode_tile(&a, &raw).unwrap().canonical_sha256,
        decode_tile(&b, &raw).unwrap().canonical_sha256
    );
}
#[test]
fn invalid_known_attributes_are_not_ground_defaults() {
    for (k, value) in [
        ("brunnel", field(1, b"ford")),
        ("access", scalar(7, 1)),
        ("subclass", scalar(4, 1)),
        ("layer", field(1, b"unknown")),
        ("oneway", scalar(7, 0)),
        ("access", field(1, "a\u{200b}b".as_bytes())),
        ("access", field(1, b"")),
        ("access", field(1, &[b'a'; 101])),
    ] {
        let raw = tile(LINE, Some((k, value)));
        assert!(decode_tile(&binding(&raw), &raw).is_err(), "{k}");
    }
    for (k, value) in [
        ("brunnel", field(1, b"bridge")),
        ("brunnel", field(1, b"tunnel")),
        ("layer", scalar(6, 1)),
        ("oneway", field(1, b"reversible")),
    ] {
        let raw = tile(LINE, Some((k, value)));
        assert!(decode_tile(&binding(&raw), &raw).is_ok(), "{k}");
    }
}
#[test]
fn malformed_geometry_never_returns_partial_lines() {
    for geom in [
        vec![],
        vec![9, 0],
        vec![10, 0, 0],
        vec![9, 0, 0],
        vec![17, 0, 0, 0, 0],
        vec![9, 0, 0, 15],
        vec![9, 0, 0, 2],
        vec![9, 0, 0, 10, 0],
        vec![9, u32::MAX as u64 + 1, 0, 10, 0, 0],
    ] {
        let raw = tile(&geom, None);
        assert!(decode_tile(&binding(&raw), &raw).is_err(), "{geom:?}");
    }
}
#[test]
fn protobuf_malformed_and_duplicate_fields_rejected() {
    let good = tile(LINE, None);
    for len in 0..good.len() {
        let raw = &good[..len];
        assert!(decode_tile(&binding(raw), raw).is_err());
    }
    for raw in [
        vec![0],
        vec![27],
        vec![0x80; 11],
        vec![26, 255, 255, 255, 255, 127],
    ] {
        assert!(decode_tile(&binding(&raw), &raw).is_err());
    }
    let l = layer(&[feature(&[0, 0], LINE)], &["class"], &[field(1, b"minor")]);
    for raw in [
        [field(3, &l), field(3, &l)].concat(),
        field(3, &[l.clone(), scalar(5, 4096)].concat()),
        field(3, &[l.clone(), field(1, b"transportation")].concat()),
    ] {
        assert!(decode_tile(&binding(&raw), &raw).is_err());
    }
    for tags in [vec![0], vec![1, 0], vec![0, 1], vec![0, 0, 0, 0]] {
        let raw = field(
            3,
            &layer(&[feature(&tags, LINE)], &["class"], &[field(1, b"minor")]),
        );
        assert!(decode_tile(&binding(&raw), &raw).is_err());
    }
}
#[test]
fn nonfinite_and_multi_valued_dictionary_entries_rejected() {
    for value in [
        [vec![25], f64::NAN.to_le_bytes().to_vec()].concat(),
        [vec![21], f32::INFINITY.to_le_bytes().to_vec()].concat(),
        [field(1, b"minor"), scalar(4, 3)].concat(),
    ] {
        let raw = field(3, &layer(&[feature(&[0, 0], LINE)], &["class"], &[value]));
        assert!(decode_tile(&binding(&raw), &raw).is_err());
    }
}
#[test]
fn absent_layer_is_not_an_empty_road_success() {
    assert_eq!(
        decode_tile(&binding(&[]), &[]),
        Err(Error::MissingTransportation)
    );
    let raw = field(3, &layer(&[], &[], &[]));
    assert!(decode_tile(&binding(&raw), &raw).unwrap().lines.is_empty());
}
#[test]
fn request_limits_and_revisions_are_checked_before_decode() {
    let huge = vec![0; MAX_TILE_BYTES + 1];
    assert_eq!(decode_tile(&binding(&huge), &huge), Err(Error::Limit));
    let raw = vec![0; MAX_REQUEST_BYTES / 2 + 1];
    let b = binding(&raw);
    let mut c = b.clone();
    c.x += 1;
    c.canonical_url = c.canonical_url.replace("14553", "14554");
    assert_eq!(
        decode_request(&[
            EvidenceInput {
                binding: &b,
                raw: &raw
            },
            EvidenceInput {
                binding: &c,
                raw: &raw
            }
        ]),
        Err(Error::Limit)
    );
    let raw = tile(LINE, None);
    let b = binding(&raw);
    assert_eq!(
        decode_request(&[
            EvidenceInput {
                binding: &b,
                raw: &raw
            },
            EvidenceInput {
                binding: &b,
                raw: &raw
            }
        ]),
        Err(Error::Binding)
    );
    let mut c = b.clone();
    c.revision = "next".into();
    c.canonical_url = c.canonical_url.replace(&b.revision, "next");
    assert_eq!(
        decode_request(&[
            EvidenceInput {
                binding: &b,
                raw: &raw
            },
            EvidenceInput {
                binding: &c,
                raw: &raw
            }
        ]),
        Err(Error::Binding)
    );
    assert_eq!(decode_request(&[]), Err(Error::Limit));
    let inputs: Vec<_> = (0..17)
        .map(|_| EvidenceInput {
            binding: &b,
            raw: &raw,
        })
        .collect();
    assert_eq!(decode_request(&inputs), Err(Error::Limit));
}
#[test]
fn point_feature_dictionary_and_command_budgets() {
    let raw = tile(&[9, 0, 0, ((20001u64) << 3) | 2], None);
    assert_eq!(decode_tile(&binding(&raw), &raw), Err(Error::Limit));
    let raw = field(
        3,
        &layer(
            &vec![feature(&[0, 0], LINE); 10001],
            &["class"],
            &[field(1, b"minor")],
        ),
    );
    assert_eq!(decode_tile(&binding(&raw), &raw), Err(Error::Limit));
    let mut l = layer(&[], &[], &[]);
    for _ in 0..20001 {
        l.extend(field(4, &scalar(4, 0)));
    }
    let raw = field(3, &l);
    assert_eq!(decode_tile(&binding(&raw), &raw), Err(Error::Limit));
}
#[test]
fn unknown_fields_are_bounded_and_ignored() {
    let raw = tile(LINE, None);
    let mut extended = raw.clone();
    extended.extend(field(20, b"unused"));
    assert_eq!(
        decode_tile(&binding(&raw), &raw).unwrap().lines,
        decode_tile(&binding(&extended), &extended).unwrap().lines
    );
}
#[test]
#[ignore = "External real tile corpus, no data download in tests"]
fn real_tile_corpus() {
    let p = std::env::var("OFM_TEST_TILE").expect("OFM_TEST_TILE path");
    let raw = std::fs::read(p).unwrap();
    let out = decode_tile(&binding(&raw), &raw).unwrap();
    println!(
        "lines={} points={} hash={}",
        out.lines.len(),
        out.lines.iter().map(|l| l.coordinates.len()).sum::<usize>(),
        out.canonical_sha256
    );
    assert_eq!(out.lines.len(), 3165);
    assert_eq!(
        out.lines.iter().map(|l| l.coordinates.len()).sum::<usize>(),
        10461
    );
}
#[test]
fn invalid_wire_type_oversized_varint_and_coordinate_accumulation() {
    for raw in [
        vec![26, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 2],
        vec![24, 0],
        vec![29, 0, 0, 0, 0],
    ] {
        assert_eq!(decode_tile(&binding(&raw), &raw), Err(Error::Malformed));
    }
    // Accumulated coordinates leave the bounded supplier buffer; no wrapping or clipping.
    let raw = tile(&[9, 131072, 0, 10, 2, 0], None);
    assert_eq!(decode_tile(&binding(&raw), &raw), Err(Error::Limit));
    let mut l = layer(&[feature(&[0, 0], LINE)], &["class"], &[field(1, b"minor")]);
    // Last-one-wins protobuf semantics must not bypass extent/version validation.
    l.extend(scalar(15, 1));
    let raw = field(3, &l);
    assert_eq!(decode_tile(&binding(&raw), &raw), Err(Error::Malformed));
}
#[test]
fn bounded_adversarial_mutations_do_not_panic_or_depend_on_history() {
    let seed = tile(LINE, Some(("brunnel", field(1, b"bridge"))));
    let mut random = 0x91ac_1234u32;
    for i in 0..2000 {
        let mut raw = seed.clone();
        for _ in 0..1 + i % 5 {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let idx = random as usize % raw.len();
            raw[idx] ^= (random >> 24) as u8;
        }
        let b = binding(&raw);
        assert_eq!(decode_tile(&b, &raw), decode_tile(&b, &raw));
    }
}
#[test]
fn complete_request_is_all_or_nothing() {
    let raw = tile(LINE, None);
    let good = binding(&raw);
    let mut bad = good.clone();
    bad.x += 1;
    bad.canonical_url = bad.canonical_url.replace("14553", "14554");
    bad.raw_sha256 = "0".repeat(64);
    assert_eq!(
        decode_request(&[
            EvidenceInput {
                binding: &good,
                raw: &raw
            },
            EvidenceInput {
                binding: &bad,
                raw: &raw
            }
        ]),
        Err(Error::Hash)
    );
    assert!(decode_tile(&good, &raw).is_ok());
}
fn raw_entry(raw: &[u8]) -> RawEvidence {
    RawEvidence {
        sha256: raw_sha256(raw),
        bytes: raw.to_vec(),
    }
}
#[test]
fn store_shares_raw_but_binds_decoded_context_to_each_tile() {
    let raw = tile(LINE, None);
    let a = binding(&raw);
    let mut b = a.clone();
    b.x += 1;
    b.canonical_url = b.canonical_url.replace("14553", "14554");
    let store =
        EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![a.clone(), b.clone()]).unwrap();
    assert_eq!(store.blobs().count(), 1);
    assert_eq!(store.bindings().count(), 2);
    assert_ne!(
        store.decoded(&a.canonical_url).unwrap().canonical_sha256,
        store.decoded(&b.canonical_url).unwrap().canonical_sha256
    );
    let clone = store.clone();
    assert!(std::ptr::eq(
        store.raw(&a.raw_sha256).unwrap(),
        clone.raw(&a.raw_sha256).unwrap()
    ));
    assert!(std::ptr::eq(
        store.decoded(&a.canonical_url).unwrap(),
        clone.decoded(&a.canonical_url).unwrap()
    ));
    let again = store
        .with_additions(vec![raw_entry(&raw)], vec![a.clone()])
        .unwrap();
    assert_eq!(again.descriptors(), store.descriptors());
    assert!(std::ptr::eq(
        store.raw(&a.raw_sha256).unwrap(),
        again.raw(&a.raw_sha256).unwrap()
    ));
    assert!(std::ptr::eq(
        store.decoded(&a.canonical_url).unwrap(),
        again.decoded(&a.canonical_url).unwrap()
    ));
}
#[test]
fn store_rejects_conflicts_missing_or_orphan_raw_without_mutating_prior_state() {
    let raw = tile(LINE, None);
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b.clone()]).unwrap();
    let before = store.descriptors();
    let mut forged = raw_entry(&raw);
    forged.bytes[0] ^= 1;
    assert!(store.with_additions(vec![forged], vec![]).is_err());
    let other = tile(LINE, Some(("oneway", field(1, b"yes"))));
    let mut new = b.clone();
    new.raw_sha256 = raw_sha256(&other);
    assert!(store
        .with_additions(vec![raw_entry(&other)], vec![new.clone()])
        .is_err());
    assert!(EvidenceStore::from_parts(vec![], vec![new]).is_err());
    assert!(store
        .with_additions(vec![raw_entry(&other)], vec![])
        .is_err());
    assert!(store.decoded("missing").is_err());
    assert!(store.raw("missing").is_err());
    assert_eq!(before, store.descriptors());
    assert_eq!(store.raw(&b.raw_sha256).unwrap(), raw);
}
#[test]
fn store_restoration_redecodes_raw_instead_of_trusting_modified_descriptors() {
    let raw = tile(LINE, None);
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b.clone()]).unwrap();
    let mut dto = store.descriptors();
    dto[0].canonical_sha256 = "forged".into();
    let restored = EvidenceStore::from_parts(
        store
            .blobs()
            .map(|(sha, bytes)| RawEvidence {
                sha256: sha.into(),
                bytes: bytes.into(),
            })
            .collect(),
        dto.into_iter().map(|d| d.binding).collect(),
    )
    .unwrap();
    assert_eq!(restored.descriptors(), store.descriptors());
    let mut bindings: Vec<_> = store.bindings().cloned().collect();
    bindings[0].revision = "tampered".into();
    assert!(EvidenceStore::from_parts(vec![raw_entry(&raw)], bindings).is_err());
}
#[test]
fn store_addition_is_atomic_and_counts_reused_blob_bytes_and_tiles() {
    let mut raw = tile(LINE, None);
    raw.extend(field(20, &vec![0; MAX_REQUEST_BYTES / 2]));
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b.clone()]).unwrap();
    let mut c = b.clone();
    c.x += 1;
    c.canonical_url = c.canonical_url.replace("14553", "14554");
    assert!(matches!(
        store.with_additions(vec![], vec![c]),
        Err(Error::Limit)
    ));
    assert_eq!(store.bindings().count(), 1);
    let raw = tile(LINE, None);
    let b = binding(&raw);
    let bindings: Vec<_> = (0..16)
        .map(|x| {
            let mut v = b.clone();
            v.x = x;
            v.canonical_url = format!(
                "https://tiles.openfreemap.org/planet/{}/14/{}/{}.pbf",
                v.revision, x, v.y
            );
            v
        })
        .collect();
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], bindings).unwrap();
    assert!(matches!(
        store.with_additions(vec![], vec![b]),
        Err(Error::Limit)
    ));
    assert_eq!(store.bindings().count(), 16);
    assert_eq!(EvidenceStore::empty().bindings().count(), 0);
}
#[test]
fn store_rejects_mixed_versions_and_partial_batch_publication() {
    let raw = tile(LINE, None);
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b.clone()]).unwrap();
    let before = store.descriptors();
    let mut next = b.clone();
    next.revision = "next".into();
    next.canonical_url = next.canonical_url.replace(&b.revision, "next");
    assert!(matches!(
        store.with_additions(vec![], vec![next]),
        Err(Error::Binding)
    ));
    let malformed = vec![26, 255];
    let mut bad = b.clone();
    bad.x = 1;
    bad.canonical_url = bad.canonical_url.replace("14553", "1");
    bad.raw_sha256 = raw_sha256(&malformed);
    let mut good = b.clone();
    good.x = 2;
    good.canonical_url = good.canonical_url.replace("14553", "2");
    assert!(store
        .with_additions(vec![raw_entry(&malformed)], vec![good, bad])
        .is_err());
    assert_eq!(store.descriptors(), before);
    assert_eq!(store.bindings().count(), 1);
}
fn xy_geometry(points: &[[i32; 2]]) -> Vec<u64> {
    let zig = |n: i32| ((i64::from(n) << 1) ^ (i64::from(n) >> 31)) as u64;
    let mut out = vec![
        9,
        zig(points[0][0]),
        zig(points[0][1]),
        (((points.len() - 1) as u64) << 3) | 2,
    ];
    for e in points.windows(2) {
        out.extend([zig(e[1][0] - e[0][0]), zig(e[1][1] - e[0][1])]);
    }
    out
}
fn seam_store(
    a: &[[i32; 2]],
    bs: &[Vec<[i32; 2]>],
    extra_a: Option<(&str, Vec<u8>)>,
    extra_b: Option<(&str, Vec<u8>)>,
) -> EvidenceStore {
    let west = tile(&xy_geometry(a), extra_a);
    let mut east_keys = vec!["class"];
    let mut east_values = vec![field(1, b"minor")];
    let mut tags = vec![0, 0];
    if let Some((k, value)) = extra_b {
        east_keys.push(k);
        east_values.push(value);
        tags.extend([1, 1]);
    }
    let east = field(
        3,
        &layer(
            &bs.iter()
                .map(|p| feature(&tags, &xy_geometry(p)))
                .collect::<Vec<_>>(),
            &east_keys,
            &east_values,
        ),
    );
    let b = binding(&east);
    let mut a = binding(&west);
    a.x = 14552;
    a.canonical_url = a.canonical_url.replace("14553", "14552");
    EvidenceStore::from_parts(vec![raw_entry(&west), raw_entry(&east)], vec![a, b]).unwrap()
}
const WEST_LINE: &[[i32; 2]] = &[[4000, 2000], [4200, 2000]];
fn east_line() -> Vec<[i32; 2]> {
    vec![[-96, 2001], [104, 2001]]
}
const SEAM_TILES: &[(u32, u32)] = &[(14552, 6451), (14553, 6451)];
#[test]
fn raw_context_candidates_cannot_be_omitted_forged_or_reversed() {
    let store = seam_store(WEST_LINE, &[east_line()], None, None);
    let actual = boundary_candidates(&store, SEAM_TILES).unwrap();
    assert_eq!(actual.len(), 1);
    assert!(actual[0].max_separation_meters > 0.3 && actual[0].max_separation_meters < 0.8);
    assert!(validate_boundary_candidates(&store, SEAM_TILES, &actual).is_ok());
    assert!(validate_boundary_candidates(&store, SEAM_TILES, &[]).is_err());
    let mut wrong = actual.clone();
    wrong[0].max_separation_meters = 0.;
    assert!(validate_boundary_candidates(&store, SEAM_TILES, &wrong).is_err());
    let mut wrong = actual.clone();
    let pair = &mut wrong[0];
    std::mem::swap(&mut pair.a_fragment, &mut pair.b_fragment);
    assert!(validate_boundary_candidates(&store, SEAM_TILES, &wrong).is_err());
    assert!(boundary_candidates(&store, &SEAM_TILES[..1]).is_err());
    assert!(boundary_candidates(&store, &[SEAM_TILES[0], SEAM_TILES[0]]).is_err());
    let mut reversed: Vec<_> = store.bindings().cloned().collect();
    reversed.reverse();
    let reload = EvidenceStore::from_parts(
        store
            .blobs()
            .map(|(sha, raw)| RawEvidence {
                sha256: sha.into(),
                bytes: raw.into(),
            })
            .collect(),
        reversed,
    )
    .unwrap();
    assert_eq!(boundary_candidates(&reload, SEAM_TILES).unwrap(), actual);
}
#[test]
fn candidate_ambiguity_structure_access_and_raw_direction_remain_distinct() {
    let duplicated = seam_store(WEST_LINE, &[east_line(), east_line()], None, None);
    assert!(boundary_candidates(&duplicated, SEAM_TILES)
        .unwrap()
        .is_empty());
    for (a, b) in [
        (
            Some(("access", field(1, b"yes"))),
            Some(("access", field(1, b"private"))),
        ),
        (Some(("layer", scalar(4, 0))), None),
        (
            Some(("brunnel", field(1, b"bridge"))),
            Some(("brunnel", field(1, b"bridge"))),
        ),
        (
            Some(("oneway", field(1, b"1"))),
            Some(("oneway", field(1, b"1"))),
        ),
        (Some(("layer", scalar(4, 1))), Some(("layer", scalar(4, 1)))),
    ] {
        assert!(
            boundary_candidates(&seam_store(WEST_LINE, &[east_line()], a, b), SEAM_TILES)
                .unwrap()
                .is_empty()
        );
    }
    let mut reversed = east_line();
    reversed.reverse();
    assert!(boundary_candidates(
        &seam_store(
            WEST_LINE,
            &[reversed],
            Some(("oneway", scalar(4, 1))),
            Some(("oneway", scalar(4, 1)))
        ),
        SEAM_TILES
    )
    .unwrap()
    .is_empty());
}
#[test]
fn distinct_parallel_roads_and_reentrant_crossings_are_not_paired() {
    let far = vec![[-96, 2005], [104, 2005]];
    assert!(
        boundary_candidates(&seam_store(WEST_LINE, &[far], None, None), SEAM_TILES)
            .unwrap()
            .is_empty()
    );
    let looped = vec![[-96, 2001], [104, 2001], [-96, 2001]];
    assert!(
        boundary_candidates(&seam_store(WEST_LINE, &[looped], None, None), SEAM_TILES)
            .unwrap()
            .is_empty()
    );
}
#[test]
#[ignore = "External real two-tile corpus, no data download in tests"]
fn real_boundary_candidate_corpus() {
    let west = std::fs::read(std::env::var("OFM_TEST_WEST").unwrap()).unwrap();
    let east = std::fs::read(std::env::var("OFM_TEST_TILE").unwrap()).unwrap();
    let b = binding(&east);
    let mut a = binding(&west);
    a.x = 14552;
    a.canonical_url = a.canonical_url.replace("14553", "14552");
    let store =
        EvidenceStore::from_parts(vec![raw_entry(&west), raw_entry(&east)], vec![a, b]).unwrap();
    let pairs = boundary_candidates(&store, SEAM_TILES).unwrap();
    let golden: Vec<(String, String, f64, f64)> =
        serde_json::from_str(include_str!("fixtures/ofm_boundary_candidate_golden.json")).unwrap();
    assert_eq!(pairs.len(), golden.len());
    for p in &pairs {
        let g = golden
            .iter()
            .find(|g| g.0 == p.a_fragment && g.1 == p.b_fragment)
            .unwrap();
        assert!((p.overlap_meters - g.2).abs() < 1e-6);
        assert!((p.max_separation_meters - g.3).abs() < 1e-6);
    }
    println!(
        "CANDIDATES {}",
        serde_json::to_string(
            &pairs
                .iter()
                .map(|p| (
                    &p.a_fragment,
                    &p.b_fragment,
                    p.overlap_meters,
                    p.max_separation_meters
                ))
                .collect::<Vec<_>>()
        )
        .unwrap()
    );
    assert!(!pairs.is_empty());
}
#[test]
fn three_tile_context_keeps_independent_mutual_uniqueness_per_boundary() {
    let shapes = [
        vec![[4000, 2000], [4200, 2000]],
        vec![[-96, 2000], [4200, 2000]],
        vec![[-96, 2000], [104, 2000]],
    ];
    let raw: Vec<_> = shapes.iter().map(|p| tile(&xy_geometry(p), None)).collect();
    let bindings = raw
        .iter()
        .enumerate()
        .map(|(i, raw)| {
            let mut b = binding(raw);
            b.x = 14552 + i as u32;
            b.canonical_url = format!(
                "https://tiles.openfreemap.org/planet/{}/14/{}/{}.pbf",
                b.revision, b.x, b.y
            );
            b
        })
        .collect();
    let store =
        EvidenceStore::from_parts(raw.iter().map(|r| raw_entry(r)).collect(), bindings).unwrap();
    let pairs =
        boundary_candidates(&store, &[(14552, 6451), (14553, 6451), (14554, 6451)]).unwrap();
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].b_fragment, pairs[1].a_fragment);
    let keys = store
        .bindings()
        .map(|b| b.canonical_url.clone())
        .collect::<Vec<_>>();
    let start = PathPick {
        fragment_id: pairs[0].a_fragment.clone(),
        segment_index: 0,
        fraction: 0.,
    };
    let end = PathPick {
        fragment_id: pairs[1].b_fragment.clone(),
        segment_index: 0,
        fraction: 1.,
    };
    let path = reconstruct_path(&store, &keys, &start, &end).unwrap();
    assert_eq!(path.descriptor.ordered_spans.len(), 3);
    assert_eq!(path.descriptor.joins.len(), 2);
    assert!(path.points.windows(2).all(|e| e[0][0] < e[1][0]));
    assert!(validate_path_descriptor(&store, &path.descriptor).is_ok());
}
#[test]
fn candidate_class_context_limit_fails_closed_instead_of_filtering_away_ambiguity() {
    let raw = field(
        3,
        &layer(
            &vec![feature(&[0, 0], &xy_geometry(WEST_LINE)); 2001],
            &["class"],
            &[field(1, b"minor")],
        ),
    );
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b]).unwrap();
    assert_eq!(
        boundary_candidates(&store, &[(14553, 6451)]),
        Err(Error::Limit)
    );
}
fn seam_picks(store: &EvidenceStore) -> (Vec<String>, PathPick, PathPick) {
    let pair = boundary_candidates(store, SEAM_TILES).unwrap().remove(0);
    (
        store.bindings().map(|b| b.canonical_url.clone()).collect(),
        PathPick {
            fragment_id: pair.a_fragment,
            segment_index: 0,
            fraction: 0.,
        },
        PathPick {
            fragment_id: pair.b_fragment,
            segment_index: 0,
            fraction: 1.,
        },
    )
}
#[test]
fn reconstructed_blend_is_continuous_reversible_and_revalidated_after_deserialize() {
    let store = seam_store(WEST_LINE, &[east_line()], None, None);
    let (keys, start, end) = seam_picks(&store);
    let path = reconstruct_path(&store, &keys, &start, &end).unwrap();
    assert_eq!(path.points.len(), 23);
    assert_eq!(path.descriptor.ordered_spans.len(), 2);
    assert_eq!(path.descriptor.joins.len(), 1);
    assert!(path.max_correction_meters <= 0.8);
    assert!(path.points.windows(2).all(|e| e[1][0] > e[0][0]));
    let reverse = reconstruct_path(&store, &keys, &end, &start).unwrap();
    assert_eq!(path.points.len(), reverse.points.len());
    for (a, b) in path.points.iter().zip(reverse.points.iter().rev()) {
        assert!((a[0] - b[0]).abs() < 1e-10 && (a[1] - b[1]).abs() < 1e-10);
    }
    let restored: PathDescriptor =
        serde_json::from_str(&serde_json::to_string(&path.descriptor).unwrap()).unwrap();
    assert_eq!(validate_path_descriptor(&store, &restored).unwrap(), path);
    for i in 0..8 {
        let mut bad = restored.clone();
        match i {
            0 => {
                bad.ordered_spans.remove(0);
            }
            1 => {
                bad.ordered_spans[0].from += 0.01;
            }
            2 => {
                bad.ordered_spans[0].binding_key = "forged".into();
            }
            3 => {
                bad.joins[0].a_range[0] += 0.01;
            }
            4 => {
                bad.joins[0].from_axis += 1e-5;
            }
            5 => {
                bad.coverage_verified = true;
            }
            6 => {
                bad.joins[0].a_id = "forged".into();
            }
            _ => {
                bad.ordered_spans[0].to = f64::NAN;
            }
        };
        assert!(validate_path_descriptor(&store, &bad).is_err());
    }
}
#[test]
fn picks_inside_blend_out_of_bounds_and_ambiguous_context_reject() {
    let store = seam_store(WEST_LINE, &[east_line()], None, None);
    let (keys, start, end) = seam_picks(&store);
    let mut bad = start.clone();
    bad.fraction = 0.5;
    assert!(reconstruct_path(&store, &keys, &bad, &end).is_err());
    bad.fraction = f64::NAN;
    assert!(reconstruct_path(&store, &keys, &bad, &end).is_err());
    bad.fraction = 0.;
    bad.segment_index = usize::MAX;
    assert!(reconstruct_path(&store, &keys, &bad, &end).is_err());
    assert!(reconstruct_path(&store, &keys, &start, &start).is_err());
    let ambiguous = seam_store(WEST_LINE, &[east_line(), east_line()], None, None);
    assert!(reconstruct_path(&ambiguous, &keys, &start, &end).is_err());
}
#[test]
#[ignore = "External real two-tile corpus, no data download in tests"]
fn real_derived_path_matches_typescript_golden() {
    let west = std::fs::read(std::env::var("OFM_TEST_WEST").unwrap()).unwrap();
    let east = std::fs::read(std::env::var("OFM_TEST_TILE").unwrap()).unwrap();
    let b = binding(&east);
    let mut a = binding(&west);
    a.x = 14552;
    a.canonical_url = a.canonical_url.replace("14553", "14552");
    let store =
        EvidenceStore::from_parts(vec![raw_entry(&west), raw_entry(&east)], vec![a, b]).unwrap();
    #[derive(serde::Deserialize)]
    struct Golden {
        descriptor: PathDescriptor,
        points: Vec<[f64; 2]>,
    }
    let golden: Golden =
        serde_json::from_str(include_str!("fixtures/ofm_derived_path_golden.json")).unwrap();
    let actual = validate_path_descriptor(&store, &golden.descriptor).unwrap();
    assert_eq!(actual.points.len(), golden.points.len());
    let mut maximum: f64 = 0.;
    for (a, b) in actual.points.iter().zip(&golden.points) {
        for i in 0..2 {
            maximum = maximum.max((a[i] - b[i]).abs());
        }
    }
    assert!(maximum < 1e-10);
    println!(
        "path points={} max_degree_difference={maximum}",
        actual.points.len()
    );
}
#[test]
fn whole_covered_source_segment_budget_is_not_bypassed_by_small_blend() {
    let dense: Vec<_> = (0..501).map(|i| [i * 12, 2000]).collect();
    let store = seam_store(&dense, &[east_line()], None, None);
    let (keys, start, end) = seam_picks(&store);
    assert_eq!(
        reconstruct_path(&store, &keys, &start, &end),
        Err(Error::Limit)
    );
}
#[test]
fn world_runtime_evidence_is_not_an_ipc_or_plain_json_backup() {
    use rosenfu_core::world::{World, WorldDraft};
    let raw = tile(LINE, None);
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b.clone()]).unwrap();
    let draft = WorldDraft::default()
        .with_road_evidence(store.clone())
        .unwrap();
    let world = World::new(draft).unwrap();
    let before = world.clone();
    let json = serde_json::to_string(world.snapshot()).unwrap();
    assert!(!json.contains("runtime_road_evidence"));
    assert!(!json.contains("coordinates"));
    assert!(json.contains("road_evidence_descriptors"));
    let mut detached: WorldDraft = serde_json::from_str(&json).unwrap();
    assert!(World::new(detached.clone()).is_err());
    assert!(detached
        .restore_road_evidence(EvidenceStore::empty())
        .is_err());
    assert_eq!(detached.runtime_road_evidence, EvidenceStore::empty());
    let restored_store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b]).unwrap();
    assert_eq!(store, restored_store);
    detached.restore_road_evidence(restored_store).unwrap();
    assert_eq!(World::new(detached).unwrap(), world);
    let ipc: WorldDraft = serde_json::from_str(&json).unwrap();
    assert_eq!(world.replace_retaining_road_evidence(ipc).unwrap(), world);
    let mut forged: WorldDraft = serde_json::from_str(&json).unwrap();
    forged.road_evidence_descriptors.clear();
    assert!(world.replace_retaining_road_evidence(forged).is_err());
    assert_eq!(world, before);
}
fn parked_ofm_plan(path: &PathDescriptor) -> rosenfu_core::world::ConstructionPlans {
    serde_json::from_value(serde_json::json!({"version":1,"plans":[{"id":1,"name":"seam"}],"parkedPaths":[{"id":2,"planId":1,"name":"seam","points":[],"redo":[],"line":[],"extension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":true,"ofmPath":path}],"activePathId":1,"activePlanId":1,"draftPoints":[],"redoPoints":[],"draftExtension":null,"trackType":"normal","designSpeedKmh":120,"roadFollowing":true,"plannedStations":[],"draftFinished":false})).unwrap()
}
fn staged_road_world() -> (
    rosenfu_core::world::World,
    rosenfu_core::world::CompanyId,
    PathDescriptor,
) {
    use rosenfu_core::world::World;
    let store = seam_store(WEST_LINE, &[east_line()], None, None);
    let (keys, start, end) = seam_picks(&store);
    let path = reconstruct_path(&store, &keys, &start, &end)
        .unwrap()
        .descriptor;
    let mut world = World::default();
    let player = world.add_player("p".into()).unwrap();
    let owner = world.add_company(player, "c".into()).unwrap();
    world
        .stage_ofm_construction_plans(
            parked_ofm_plan(&path),
            store
                .blobs()
                .map(|(sha, raw)| RawEvidence {
                    sha256: sha.into(),
                    bytes: raw.into(),
                })
                .collect(),
            store.bindings().cloned().collect(),
        )
        .unwrap();
    (world, owner, path)
}
#[test]
fn park_and_build_are_atomic_backend_geometry_and_keep_separate_path_provenance() {
    use rosenfu_core::world::{TrackType, World, WorldDraft};
    let (mut world, owner, path) = staged_road_world();
    let before = world.clone();
    let mut bad = path.clone();
    bad.joins[0].a_range[0] += 0.1;
    assert!(world
        .stage_ofm_construction_plans(parked_ofm_plan(&bad), vec![], vec![])
        .is_err());
    assert_eq!(world, before);
    let plans = world.snapshot().construction_plans.as_ref().unwrap();
    assert_eq!(plans.parked_paths[0].points.len(), 23);
    assert_eq!(plans.parked_paths[0].line, plans.parked_paths[0].points);
    let id = world
        .build_alignment_from_road_path(owner, "built".into(), TrackType::Normal, path)
        .unwrap();
    assert!(world.snapshot().alignment_provenance.is_empty());
    assert_eq!(world.snapshot().road_path_provenance.len(), 1);
    assert_eq!(world.snapshot().road_path_provenance[0].alignment_id, id);
    let built = world.clone();
    let mut moved = world.snapshot().clone();
    moved.alignments[0].points[5].latitude += 0.001;
    assert!(world.replace(moved).is_err());
    assert_eq!(world, built);
    let mut detached: WorldDraft =
        serde_json::from_str(&serde_json::to_string(world.snapshot()).unwrap()).unwrap();
    assert!(World::new(detached.clone()).is_err());
    detached
        .restore_road_evidence(world.snapshot().runtime_road_evidence.clone())
        .unwrap();
    assert_eq!(World::new(detached).unwrap(), world);
    let mut deleted = world.snapshot().clone();
    deleted.alignments.clear();
    world.replace(deleted).unwrap();
    assert!(world.snapshot().road_path_provenance.is_empty());
    assert_eq!(
        world.snapshot().runtime_road_evidence,
        built.snapshot().runtime_road_evidence
    );
    world = World::new(built.snapshot().clone()).unwrap();
    assert_eq!(world, built);
}
#[test]
fn path_provenance_survives_insert_split_and_endpoint_growth() {
    use rosenfu_core::world::{AlignmentEndpoint, GeoPoint, TrackType};
    let (mut world, owner, path) = staged_road_world();
    let id = world
        .build_alignment_from_road_path(owner, "built".into(), TrackType::Normal, path.clone())
        .unwrap();
    let before = world.clone();
    let suffix = world
        .split_alignment_at_position(owner, id, 10, 0.5)
        .unwrap();
    assert_eq!(world.snapshot().road_path_provenance.len(), 2);
    assert!(world
        .snapshot()
        .road_path_provenance
        .iter()
        .any(|p| p.alignment_id == suffix));
    world.snapshot().validate().unwrap();
    assert_ne!(world, before);
    for endpoint in [AlignmentEndpoint::End, AlignmentEndpoint::Start] {
        let (mut w, owner, path) = staged_road_world();
        let store = &w.snapshot().runtime_road_evidence;
        let actual = validate_path_descriptor(store, &path).unwrap();
        let p = if endpoint == AlignmentEndpoint::End {
            actual.points[0]
        } else {
            *actual.points.last().unwrap()
        };
        let a = GeoPoint {
            longitude: p[0],
            latitude: p[1],
        };
        let outside = GeoPoint {
            longitude: p[0]
                + if endpoint == AlignmentEndpoint::End {
                    -0.0004
                } else {
                    0.0004
                },
            latitude: p[1],
        };
        let points = if endpoint == AlignmentEndpoint::End {
            vec![outside, a]
        } else {
            vec![a, outside]
        };
        let id = w
            .build_alignment_typed(owner, "extension".into(), points, TrackType::Normal)
            .unwrap();
        let path = if endpoint == AlignmentEndpoint::Start {
            reconstruct_path(
                &w.snapshot().runtime_road_evidence,
                &path.required_binding_keys,
                &path.end,
                &path.start,
            )
            .unwrap()
            .descriptor
        } else {
            path
        };
        w.extend_alignment_from_road_path(owner, id, endpoint, path)
            .unwrap();
        w.snapshot().validate().unwrap();
        assert_eq!(w.snapshot().road_path_provenance.len(), 1);
    }
}
#[test]
fn immutable_scope_keeps_old_path_valid_after_unrelated_tile_addition() {
    let (mut world, _, path) = staged_road_world();
    let before = world.snapshot().clone();
    let raw = tile(LINE, None);
    let mut b = binding(&raw);
    b.x = 100;
    b.y = 100;
    b.canonical_url = format!(
        "https://tiles.openfreemap.org/planet/{}/14/100/100.pbf",
        b.revision
    );
    world
        .stage_ofm_construction_plans(parked_ofm_plan(&path), vec![raw_entry(&raw)], vec![b])
        .unwrap();
    world.snapshot().validate().unwrap();
    assert_eq!(world.snapshot().runtime_road_evidence.bindings().count(), 3);
    let scoped = world
        .snapshot()
        .runtime_road_evidence
        .scope(&path.required_binding_keys)
        .unwrap();
    assert_eq!(scoped, before.runtime_road_evidence);
    assert!(world
        .snapshot()
        .runtime_road_evidence
        .scope(&["missing".into()])
        .is_err());
    assert!(scoped
        .scope(&[
            path.required_binding_keys[0].clone(),
            path.required_binding_keys[0].clone()
        ])
        .is_err());
}
#[test]
fn whole_tile_scope_cannot_hide_same_tile_ambiguous_fragments() {
    let ambiguous = seam_store(WEST_LINE, &[east_line(), east_line()], None, None);
    let keys = ambiguous
        .bindings()
        .map(|b| b.canonical_url.clone())
        .collect::<Vec<_>>();
    let scoped = ambiguous.scope(&keys).unwrap();
    assert!(boundary_candidates(&scoped, SEAM_TILES).unwrap().is_empty());
    let only_west = ambiguous.scope(&keys[..1]).unwrap();
    assert!(boundary_candidates(&only_west, &SEAM_TILES[..1])
        .unwrap()
        .is_empty());
    assert!(boundary_candidates(&only_west, SEAM_TILES).is_err());
}
#[test]
fn existing_path_provenance_survives_plain_append_prepend_and_forbids_ui_removal() {
    use rosenfu_core::world::{AlignmentEndpoint, GeoPoint, TrackType};
    for endpoint in [AlignmentEndpoint::Start, AlignmentEndpoint::End] {
        let (mut world, owner, path) = staged_road_world();
        let id = world
            .build_alignment_from_road_path(owner, "p".into(), TrackType::Normal, path)
            .unwrap();
        let alignment = &world.snapshot().alignments[0];
        let p = if endpoint == AlignmentEndpoint::Start {
            alignment.points[0]
        } else {
            *alignment.points.last().unwrap()
        };
        let outside = GeoPoint {
            longitude: p.longitude
                + if endpoint == AlignmentEndpoint::Start {
                    -0.0004
                } else {
                    0.0004
                },
            latitude: p.latitude,
        };
        world
            .extend_alignment(owner, id, endpoint, vec![p, outside])
            .unwrap();
        world.snapshot().validate().unwrap();
        assert_eq!(
            world.snapshot().road_path_provenance[0].start_point_index,
            usize::from(endpoint == AlignmentEndpoint::Start)
        );
        let mut forged = world.snapshot().clone();
        forged.road_path_provenance.clear();
        assert!(world.replace_retaining_road_evidence(forged).is_err());
    }
}
#[test]
fn portable_session_fixture_matches_validated_synthetic_world() {
    let (world, _, path) = staged_road_world();
    let store = &world.snapshot().runtime_road_evidence;
    let value = serde_json::json!({"rawEvidence":store.blobs().map(|(sha,raw)|serde_json::json!({"sha256":sha,"bytes":raw})).collect::<Vec<_>>(),"tileBindings":store.bindings().collect::<Vec<_>>(),"path":path,"plans":world.snapshot().construction_plans});
    if std::env::var_os("OFM_UPDATE_SESSION_FIXTURE").is_some() {
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/ofm_session_synthetic.json"
            ),
            serde_json::to_string_pretty(&value).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ofm_session_synthetic.json")).unwrap();
    assert_eq!(value, expected);
}
