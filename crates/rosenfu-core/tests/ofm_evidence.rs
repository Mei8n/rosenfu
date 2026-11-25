use rosenfu_core::ofm_evidence::*;
use rosenfu_core::world::WorldDraft;
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
        ("brunnel", field(1, b"unknown")),
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

#[test]
fn road_path_height_profiles_keep_provenance_and_reject_excess_grade_atomically() {
    use rosenfu_core::world::{TrackType, VerticalProfile};
    for track in [TrackType::Normal, TrackType::Tram, TrackType::HighSpeed] {
        let store = seam_store(
            &[[3000, 2000], [5000, 2000]],
            &[vec![[-1000, 2001], [1000, 2001]]],
            None,
            None,
        );
        let (keys, start, end) = seam_picks(&store);
        let path = reconstruct_path(&store, &keys, &start, &end)
            .unwrap()
            .descriptor;
        let mut world = rosenfu_core::world::World::new(
            rosenfu_core::world::WorldDraft::default()
                .with_road_evidence(store)
                .unwrap(),
        )
        .unwrap();
        let player = world.add_player("p".into()).unwrap();
        let owner = world.add_company(player, "c".into()).unwrap();
        let id = world
            .build_alignment_from_road_path(owner, "seam".into(), track, path)
            .unwrap();
        let provenance = world.snapshot().road_path_provenance.clone();
        let mut draft = world.snapshot().clone();
        let a = draft.alignments.iter_mut().find(|a| a.id == id).unwrap();
        a.anchor_indices = Some(vec![0, a.points.len() - 1]);
        a.section_profiles = vec![VerticalProfile {
            start_height_m: 0.,
            end_height_m: 6.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        }];
        world.replace(draft).unwrap();
        assert_eq!(world.snapshot().road_path_provenance, provenance);
        let before = world.clone();
        let mut bad = world.snapshot().clone();
        let a = bad.alignments.iter_mut().find(|a| a.id == id).unwrap();
        a.section_profiles[0].end_height_m = a.length_m();
        assert!(world.replace(bad).is_err());
        assert_eq!(world, before);
        assert_eq!(
            track.maximum_construction_gradient_per_mille(),
            if track == TrackType::HighSpeed {
                30.
            } else {
                45.
            }
        );
    }
}

#[test]
fn road_path_exact_endpoint_connection_requires_actual_height_and_preserves_assets() {
    use rosenfu_core::world::{
        AlignmentEndpoint, AlignmentEndpointRef, GeoPoint, TrackType, VerticalProfile,
    };
    let (mut world, owner, path) = staged_road_world();
    let id = world
        .build_alignment_from_road_path(owner, "seam".into(), TrackType::Normal, path)
        .unwrap();
    let a = world
        .snapshot()
        .alignments
        .iter()
        .find(|a| a.id == id)
        .unwrap();
    let p = a.points[0];
    let next = a.points[1];
    let outside = GeoPoint {
        longitude: p.longitude + (p.longitude - next.longitude) * 10.,
        latitude: p.latitude + (p.latitude - next.latitude) * 10.,
    };
    let base = world
        .build_alignment(owner, "existing".into(), vec![outside, p])
        .unwrap();
    let mut elevated = world.snapshot().clone();
    elevated
        .alignments
        .iter_mut()
        .find(|a| a.id == id)
        .unwrap()
        .vertical_profile = Some(VerticalProfile {
        start_height_m: 6.,
        end_height_m: 6.,
        start_tangent_m: 0.,
        end_tangent_m: 0.,
    });
    world.replace(elevated).unwrap();
    let source = AlignmentEndpointRef {
        alignment_id: base,
        endpoint: AlignmentEndpoint::End,
    };
    let target = AlignmentEndpointRef {
        alignment_id: id,
        endpoint: AlignmentEndpoint::Start,
    };
    let before = world.clone();
    assert!(world
        .connect_alignment_endpoints(owner, source, target)
        .is_err());
    assert_eq!(world, before);
    let mut aligned = world.snapshot().clone();
    aligned
        .alignments
        .iter_mut()
        .find(|a| a.id == base)
        .unwrap()
        .vertical_profile = Some(VerticalProfile {
        start_height_m: 6.,
        end_height_m: 6.,
        start_tangent_m: 0.,
        end_tangent_m: 0.,
    });
    world.replace(aligned).unwrap();
    let before = world.snapshot().clone();
    world
        .connect_alignment_endpoints(owner, source, target)
        .unwrap();
    assert_eq!(world.snapshot().alignments, before.alignments);
    assert_eq!(
        world.snapshot().road_path_provenance,
        before.road_path_provenance
    );
    assert_eq!(
        world.snapshot().station_placements,
        before.station_placements
    );
    assert_eq!(world.snapshot().route_paths, before.route_paths);
    assert_eq!(world.snapshot().service_paths, before.service_paths);
}

#[test]
fn retained_archive_keeps_the_request_byte_budget_even_when_raw_blobs_are_shared() {
    let mut raw = tile(LINE, None);
    raw.extend(field(20, &vec![0; MAX_REQUEST_BYTES / 2]));
    let a = binding(&raw);
    let mut b = a.clone();
    b.x += 1;
    b.canonical_url = b.canonical_url.replace("14553", "14554");
    let keys = [a.canonical_url.clone(), b.canonical_url.clone()];
    let archive = EvidenceStore::from_archive_parts(vec![raw_entry(&raw)], vec![a, b]).unwrap();
    assert_eq!(archive.blobs().count(), 1);
    assert!(matches!(archive.scope(&keys), Err(Error::Limit)));
    for key in &keys {
        let scope = archive.scope(std::slice::from_ref(key)).unwrap();
        let tile = scope.decoded(key).unwrap();
        assert_eq!(scope.raw(&tile.binding.raw_sha256).unwrap(), raw);
    }
}

#[test]
fn archive_and_cold_scope_rederive_identical_tiles_without_weakening_ingress() {
    let raw = tile(LINE, None);
    let a = binding(&raw);
    let mut b = a.clone();
    b.x += 1;
    b.canonical_url = b.canonical_url.replace("14553", "14554");
    let store =
        EvidenceStore::from_archive_parts(vec![raw_entry(&raw)], vec![a.clone(), b.clone()])
            .unwrap();
    assert_eq!(store.blobs().count(), 1);
    let keys = vec![a.canonical_url.clone(), b.canonical_url.clone()];
    let scope = store.scope(&keys).unwrap();
    for binding in [&a, &b] {
        assert_eq!(
            scope.decoded(&binding.canonical_url).unwrap(),
            &decode_tile(binding, &raw).unwrap()
        );
    }
    let mut corrupt = raw_entry(&raw);
    corrupt.bytes[0] ^= 1;
    assert!(matches!(
        EvidenceStore::from_archive_parts(vec![corrupt], vec![a]),
        Err(Error::Hash)
    ));
    let malformed = [255];
    assert!(EvidenceStore::from_archive_parts(
        vec![raw_entry(&malformed)],
        vec![binding(&malformed)]
    )
    .is_err());
}

fn bus_geometry(points: &[[i32; 2]]) -> Vec<u64> {
    let mut geom = Vec::new();
    let mut prev = [0i32; 2];
    for (i, &point) in points.iter().enumerate() {
        match i {
            0 => geom.push(9),
            1 => geom.push((points.len() as u64 - 1) * 8 + 2),
            _ => {}
        }
        for j in 0..2 {
            let delta = point[j] - prev[j];
            geom.push(((delta << 1) ^ (delta >> 31)) as u64);
        }
        prev = point;
    }
    geom
}

type BusFixtureLine<'a> = (&'a [[i32; 2]], Option<(&'a str, &'a str)>);
fn bus_fixture_raw(lines: &[BusFixtureLine<'_>]) -> Vec<u8> {
    let mut keys = vec!["class"];
    let mut values = vec![field(1, b"minor")];
    let mut features = vec![];
    for (points, extra) in lines {
        let mut tags = vec![0, 0];
        if let Some((k, value)) = extra {
            let index = keys.iter().position(|x| x == k).unwrap_or_else(|| {
                keys.push(k);
                keys.len() - 1
            });
            tags.extend([index as u64, values.len() as u64]);
            values.push(field(1, value.as_bytes()));
        }
        features.push(feature(&tags, &bus_geometry(points)));
    }
    field(3, &layer(&features, &keys, &values))
}

fn bus_fixture(lines: &[BusFixtureLine<'_>]) -> (TileBinding, EvidenceStore) {
    let raw = bus_fixture_raw(lines);
    let b = binding(&raw);
    let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![b.clone()]).unwrap();
    (b, store)
}

fn bus_vertex(feature: u32, vertex: u32) -> BusVertex {
    BusVertex {
        feature,
        line: 0,
        vertex,
    }
}

#[test]
fn ordinary_bus_routes_both_ways_on_verified_road_and_replays_source_identity() {
    let (b, store) = bus_fixture(&[
        (
            &[[100, 100], [200, 100], [300, 100]],
            Some(("oneway", "yes")),
        ),
        (&[[200, 100], [200, 200], [200, 300]], Some(("layer", "0"))),
    ]);
    let a = bus_vertex(0, 0);
    let z = bus_vertex(1, 2);
    let route =
        |a, z, strict| find_gameplay_bus_path(&store, &b.canonical_url, a, z, strict).unwrap();
    let forward = route(a, z, false).unwrap();
    let reverse = route(z, a, false).unwrap();
    assert_ne!(forward.source_atom_id(0), reverse.source_atom_id(2));
    assert!(forward
        .source_atom_id(0)
        .unwrap()
        .starts_with("ofm-bus:edge:"));
    assert!(forward.source_atom_id(3).is_none());
    assert_eq!(forward.points.len(), 4);
    assert_eq!(
        forward.points.iter().rev().copied().collect::<Vec<_>>(),
        reverse.points
    );
    assert!((forward.length_m - reverse.length_m).abs() < 1e-9);
    assert!((1.0..250.0).contains(&forward.length_m));
    assert_eq!(forward.spans.len(), 2);
    assert_eq!((forward.spans[0].from, forward.spans[0].to), (0., 1.));
    assert_eq!((forward.spans[1].from, forward.spans[1].to), (0., 2.));
    assert_eq!((reverse.spans[0].from, reverse.spans[0].to), (2., 0.));
    assert_eq!(forward.binding.rule, BUS_GRAPH_RULE);
    assert!(route(a, z, true).is_some());
    assert!(route(z, a, true).is_none());
    let archived = EvidenceStore::from_archive_parts(
        vec![raw_entry(store.raw(&b.raw_sha256).unwrap())],
        vec![b.clone()],
    )
    .unwrap();
    let repro = serde_json::from_str(&serde_json::to_string(&forward.binding).unwrap()).unwrap();
    assert_eq!(
        reconstruct_gameplay_bus_path(&archived.scope(&[b.canonical_url.clone()]).unwrap(), &repro)
            .unwrap(),
        Some(forward.clone())
    );
    let mut altered = repro;
    altered.tile_raw_sha256 = "0".repeat(64);
    assert_eq!(
        reconstruct_gameplay_bus_path(&store, &altered),
        Err(Error::Hash)
    );
}

#[test]
fn ordinary_bus_does_not_snap_connect_grade_or_infer_an_interior_crossing() {
    for (vertical, tag) in [
        (&[[200, 50], [200, 100], [200, 150]][..], None),
        (&[[200, 50], [200, 150]][..], None),
        (&[[201, 100], [201, 150]][..], None),
        (&[[200, 100], [200, 150]][..], Some(("brunnel", "bridge"))),
        (&[[200, 100], [200, 150]][..], Some(("brunnel", "tunnel"))),
        (&[[200, 100], [200, 150]][..], Some(("layer", "1"))),
        (&[[200, 100], [200, 150]][..], Some(("layer", "-1"))),
    ] {
        let (b, store) = bus_fixture(&[
            (&[[100, 100], [200, 100], [300, 100]], None),
            (vertical, tag),
        ]);
        assert!(
            find_gameplay_bus_path(
                &store,
                &b.canonical_url,
                bus_vertex(0, 0),
                bus_vertex(1, vertical.len() as u32 - 1),
                false,
            )
            .unwrap()
            .is_none(),
            "{vertical:?} {tag:?}"
        );
    }
}

#[test]
fn supplier_ford_does_not_poison_same_tile_but_is_not_routable() {
    let (binding, store) = bus_fixture(&[
        (&[[100, 100], [200, 100], [300, 100]], None),
        (&[[200, 100], [200, 200]], Some(("brunnel", "ford"))),
    ]);
    let raw = store.raw(&binding.raw_sha256).unwrap();
    let decoded = decode_tile(&binding, raw).unwrap();
    assert_eq!(
        decoded.lines[1].attributes["brunnel"],
        RawValue::String("ford".into())
    );
    let start = bus_vertex(0, 0);
    let ordinary = bus_vertex(0, 2);
    let ford = bus_vertex(1, 1);
    let bus =
        |end| find_gameplay_bus_path(&store, &binding.canonical_url, start, end, true).unwrap();
    let accepted = bus(ordinary).unwrap();
    assert_eq!(accepted.points.len(), 3);
    assert!(bus(ford).is_none());
    let mut structured = accepted.binding.clone();
    structured.end = ford;
    structured.grade_policy = Some(BusGradePolicy::ObservedConnectors);
    assert!(reconstruct_gameplay_bus_path(&store, &structured)
        .unwrap()
        .is_none());

    let tram = |end| {
        reconstruct_authored_tram_guide(
            &store,
            &AuthoredTramGuide {
                tile_key: binding.canonical_url.clone(),
                tile_raw_sha256: binding.raw_sha256.clone(),
                start,
                end,
                start_fraction_q: None,
                end_fraction_q: None,
                adjacent_end: None,
            },
        )
        .unwrap()
    };
    assert_eq!(tram(ordinary).unwrap(), accepted.points);
    assert!(tram(ford).is_none());
}

#[test]
fn ordinary_bus_refuses_denied_unknown_or_buffered_edges() {
    for tag in [
        Some(("access", "private")),
        Some(("access", "no")),
        Some(("access", "conditional")),
        Some(("oneway", "reversible")),
    ] {
        let (b, store) = bus_fixture(&[(&[[100, 100], [200, 100]], tag)]);
        for strict in [false, true] {
            assert!(
                find_gameplay_bus_path(
                    &store,
                    &b.canonical_url,
                    bus_vertex(0, 0),
                    bus_vertex(0, 1),
                    strict,
                )
                .unwrap()
                .is_none(),
                "{tag:?}"
            );
        }
    }
    let (b, store) = bus_fixture(&[(&[[-20, 100], [100, 100], [200, 100]], None)]);
    let query = |index| {
        find_gameplay_bus_path(
            &store,
            &b.canonical_url,
            bus_vertex(0, index),
            bus_vertex(0, 2),
            false,
        )
        .unwrap()
    };
    assert!(query(0).is_none());
    assert!(query(1).is_some());
    assert!(query(3).is_none());
}

#[test]
fn ordinary_bus_strict_reverse_and_no_cross_talk_from_an_ambiguous_third_branch() {
    let (b, store) = bus_fixture(&[
        (
            &[[100, 100], [200, 100], [300, 100]],
            Some(("oneway", "-1")),
        ),
        (&[[200, 50], [200, 100], [200, 150]], None),
        (&[[200, 100], [150, 150]], None),
    ]);
    let query = |a, z| find_gameplay_bus_path(&store, &b.canonical_url, a, z, true).unwrap();
    assert!(query(bus_vertex(0, 2), bus_vertex(0, 0)).is_some());
    assert!(query(bus_vertex(0, 0), bus_vertex(0, 2)).is_none());
    assert!(query(bus_vertex(0, 2), bus_vertex(1, 2)).is_none());
    assert!(query(bus_vertex(0, 2), bus_vertex(2, 1)).is_none());
}

fn bus_rectangle(cells: &[(u32, u32, &[BusFixtureLine<'_>])]) -> (Vec<TileBinding>, EvidenceStore) {
    let raw: Vec<_> = cells
        .iter()
        .map(|(_, _, lines)| bus_fixture_raw(lines))
        .collect();
    let bindings: Vec<_> = raw
        .iter()
        .zip(cells)
        .map(|(r, (x, y, _))| {
            let mut b = binding(r);
            b.x = *x;
            b.y = *y;
            b.canonical_url = format!(
                "https://tiles.openfreemap.org/planet/{}/14/{x}/{y}.pbf",
                b.revision
            );
            b
        })
        .collect();
    let store = EvidenceStore::from_archive_parts(
        raw.iter().map(|r| raw_entry(r)).collect(),
        bindings.clone(),
    )
    .unwrap();
    (bindings, store)
}

fn bus_rectangle_query(tiles: &[TileBinding], end: BusVertex, oneway: bool) -> BusPathBinding {
    BusPathBinding {
        rule: BUS_RECTANGLE_RULE.into(),
        same_atom_stops: None,
        fractional_terminals: None,
        grade_policy: None,
        tile_key: tiles[0].canonical_url.clone(),
        tile_raw_sha256: tiles[0].raw_sha256.clone(),
        additional_tiles: tiles[1..]
            .iter()
            .map(|t| BusTileProof {
                tile_key: t.canonical_url.clone(),
                tile_raw_sha256: t.raw_sha256.clone(),
            })
            .collect(),
        end_tile_key: Some(tiles.last().unwrap().canonical_url.clone()),
        pattern: None,
        interior_stops: vec![],
        via: vec![],
        respect_oneway: oneway,
        start: bus_vertex(0, 0),
        end,
    }
}

#[test]
fn ordinary_bus_crosses_two_exact_seams_without_raw_seam_vertices() {
    let (tiles, store) = bus_rectangle(&[
        (14552, 6451, &[(&[[3200, 2010], [4200, 2010]], None)]),
        (14553, 6451, &[(&[[-104, 2010], [4200, 2010]], None)]),
        (14554, 6451, &[(&[[-104, 2010], [896, 2010]], None)]),
    ]);
    let request = bus_rectangle_query(&tiles, bus_vertex(0, 1), false);
    let path = reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .unwrap();
    assert_eq!(path.spans.len(), 3);
    assert_eq!(path.points.len(), 4);
    assert_eq!(
        path.spans
            .iter()
            .map(|s| s.binding_key.clone())
            .collect::<Vec<_>>(),
        tiles
            .iter()
            .map(|t| t.canonical_url.clone())
            .collect::<Vec<_>>()
    );
    assert!(path.spans[0].to < 1.);
    assert!(path.spans[1].from > 0. && path.spans[1].to < 1.);
    assert!(path.spans[2].from > 0.);
    assert!((2000.0..3200.0).contains(&path.length_m));
    let mut reversed_tiles = tiles.clone();
    reversed_tiles.reverse();
    let mut reverse = bus_rectangle_query(&reversed_tiles, bus_vertex(0, 0), false);
    reverse.start = bus_vertex(0, 1);
    let result = reconstruct_gameplay_bus_path(&store, &reverse)
        .unwrap()
        .unwrap();
    assert_eq!(
        result.points,
        path.points.iter().rev().copied().collect::<Vec<_>>()
    );
    let mut paired = request.clone();
    paired.pattern = Some(rosenfu_core::ofm_evidence::BusPattern::OpenRoundTrip);
    let draft = WorldDraft {
        runtime_road_evidence: store.clone(),
        ..WorldDraft::default()
    };
    let returned = draft.gameplay_bus_return_path(&paired).unwrap().unwrap();
    assert_eq!(returned.binding.tile_key, tiles[2].canonical_url);
    assert_eq!(returned.binding.tile_raw_sha256, tiles[2].raw_sha256);
    assert_eq!(
        returned.binding.end_tile_key,
        Some(tiles[0].canonical_url.clone())
    );
    assert_eq!(returned.points, result.points);
    assert_eq!(returned.spans, result.spans);
    assert!(draft.gameplay_bus_return_path(&paired).unwrap().is_some());

    let mut wrong = request.clone();
    wrong.additional_tiles[0].tile_raw_sha256 = "0".repeat(64);
    assert_eq!(
        reconstruct_gameplay_bus_path(&store, &wrong),
        Err(Error::Hash)
    );
    let mut missing = request;
    missing.additional_tiles.remove(0);
    assert_eq!(
        reconstruct_gameplay_bus_path(&store, &missing),
        Err(Error::Binding)
    );
}

#[test]
fn ordinary_bus_seam_refuses_nonidentical_nearby_or_grade_separated_fragments() {
    for (offset, tag) in [
        (1, None),
        (0, Some(("brunnel", "bridge"))),
        (0, Some(("layer", "1"))),
        (0, Some(("access", "private"))),
    ] {
        let (tiles, store) = bus_rectangle(&[
            (14552, 6451, &[(&[[3200, 2010], [4200, 2010]], None)]),
            (
                14553,
                6451,
                &[(&[[-104, 2010 + offset], [896, 2010 + offset]], tag)],
            ),
        ]);
        assert!(
            reconstruct_gameplay_bus_path(
                &store,
                &bus_rectangle_query(&tiles, bus_vertex(0, 1), false)
            )
            .unwrap()
            .is_none(),
            "crossed seam despite {offset} {tag:?}"
        );
    }
}

#[test]
fn ordinary_bus_seams_preserve_selected_oneway_mode_and_complete_diagonal_rectangle() {
    // Vertical through-road. Tag 1 uses numeric MVT encoding accepted by the
    // original ground-seam verifier, rather than inferring arbitrary strings.
    let raw = [
        tile(
            &bus_geometry(&[[2000, 3500], [2000, 4196]]),
            Some(("oneway", scalar(4, 1))),
        ),
        tile(
            &bus_geometry(&[[2000, -100], [2000, 896]]),
            Some(("oneway", scalar(4, 1))),
        ),
    ];
    let mut t: Vec<_> = raw.iter().map(|r| binding(r)).collect();
    t[1].y += 1;
    t[1].canonical_url = t[1].canonical_url.replace("6451.pbf", "6452.pbf");
    let store =
        EvidenceStore::from_parts(raw.iter().map(|r| raw_entry(r)).collect(), t.clone()).unwrap();
    let forward = bus_rectangle_query(&t, bus_vertex(0, 1), true);
    assert!(reconstruct_gameplay_bus_path(&store, &forward)
        .unwrap()
        .is_some());
    t.reverse();
    let mut reverse = bus_rectangle_query(&t, bus_vertex(0, 0), true);
    reverse.start = bus_vertex(0, 1);
    assert!(reconstruct_gameplay_bus_path(&store, &reverse)
        .unwrap()
        .is_none());
    reverse.respect_oneway = false;
    assert!(reconstruct_gameplay_bus_path(&store, &reverse)
        .unwrap()
        .is_some());

    let (tiles, store) = bus_rectangle(&[
        (14552, 6451, &[(&[[3200, 2010], [4200, 2010]], None)]),
        (14553, 6452, &[(&[[-104, 2010], [896, 2010]], None)]),
    ]);
    assert_eq!(
        reconstruct_gameplay_bus_path(
            &store,
            &bus_rectangle_query(&tiles, bus_vertex(0, 1), false)
        ),
        Err(Error::Binding)
    );
}

#[test]
fn dense_bus_seam_budget_counts_crossing_candidates_not_unrelated_interiors() {
    let west = [[3200, 2010], [4200, 2010]];
    let east = [[-104, 2010], [896, 2010]];
    let short = [[100, 100], [120, 180]];
    let mut left: Vec<BusFixtureLine<'_>> = vec![(&west, None)];
    left.extend(std::iter::repeat_n((&short[..], None), 2500));
    let accepted: &[BusFixtureLine<'_>] = &[(&east, None)];
    let duplicate: &[BusFixtureLine<'_>] = &[(&east, None), (&east, None)];
    for (right, pass) in [(accepted, true), (duplicate, false)] {
        let (tiles, store) = bus_rectangle(&[(14552, 6451, &left), (14553, 6451, right)]);
        assert_eq!(
            boundary_candidates(&store, &[(14552, 6451), (14553, 6451)]),
            Err(Error::Limit)
        );
        assert_eq!(
            reconstruct_gameplay_bus_path(
                &store,
                &bus_rectangle_query(&tiles, bus_vertex(0, 1), false)
            )
            .unwrap()
            .is_some(),
            pass
        );
    }
}

#[test]
fn ordinary_bus_vias_share_one_scope_and_keep_ordered_occurrences_across_tiles() {
    let (tiles, store) = bus_rectangle(&[
        (14552, 6451, &[(&[[3200, 2010], [4200, 2010]], None)]),
        (
            14553,
            6451,
            &[(
                &[[-104, 2010], [1000, 2010], [3000, 2010], [4200, 2010]],
                None,
            )],
        ),
        (14554, 6451, &[(&[[-104, 2010], [896, 2010]], None)]),
    ]);
    let mut request = bus_rectangle_query(&tiles, bus_vertex(0, 1), false);
    let direct = reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .unwrap();
    assert!(!serde_json::to_string(&request).unwrap().contains("\"via\""));
    request.via = vec![
        BusVia {
            id: "v-a".into(),
            tile_key: tiles[1].canonical_url.clone(),
            vertex: bus_vertex(0, 1),
        },
        BusVia {
            id: "v-b".into(),
            tile_key: tiles[1].canonical_url.clone(),
            vertex: bus_vertex(0, 2),
        },
    ];
    let straight = reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .unwrap();
    assert_eq!(straight.points, direct.points);
    assert_eq!(straight.via_point_indices, [2, 3]);
    assert_eq!(straight.length_m, direct.length_m);
    for i in 0..direct.points.len() - 1 {
        assert_eq!(straight.source_atom_id(i), direct.source_atom_id(i));
    }

    // Click order is authoritative even when shortest endpoints would ignore
    // the backwards excursion at the valid bidirectional neighbor.
    request.via.swap(0, 1);
    let detour = reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .unwrap();
    assert_eq!(detour.via_point_indices, [3, 4]);
    assert!(detour.length_m > straight.length_m);
    assert_eq!(
        detour.points[detour.via_point_indices[0]],
        straight.points[3]
    );
    assert_eq!(
        detour.points[detour.via_point_indices[1]],
        straight.points[2]
    );
    let archived = serde_json::from_str(&serde_json::to_string(&request).unwrap()).unwrap();
    assert_eq!(
        reconstruct_gameplay_bus_path(&store, &archived).unwrap(),
        Some(detour)
    );

    let mut duplicate = request.clone();
    duplicate.via[1].id = duplicate.via[0].id.clone();
    assert_eq!(
        reconstruct_gameplay_bus_path(&store, &duplicate),
        Err(Error::Binding)
    );
    let mut wrong = request.clone();
    wrong.via[1].vertex = bus_vertex(0, 99);
    assert!(reconstruct_gameplay_bus_path(&store, &wrong)
        .unwrap()
        .is_none());
    let mut over = request;
    over.via = (0..31)
        .map(|i| BusVia {
            id: format!("x{i}"),
            tile_key: tiles[1].canonical_url.clone(),
            vertex: bus_vertex(0, 1),
        })
        .collect();
    assert_eq!(
        reconstruct_gameplay_bus_path(&store, &over),
        Err(Error::Limit)
    );
}

#[test]
fn ordinary_bus_via_that_requires_wrong_way_refuses_whole_draft() {
    let (tile, store) = bus_fixture(&[(
        &[[100, 100], [200, 100], [300, 100], [400, 100]],
        Some(("oneway", "yes")),
    )]);
    let mut request = find_gameplay_bus_path(
        &store,
        &tile.canonical_url,
        bus_vertex(0, 0),
        bus_vertex(0, 3),
        true,
    )
    .unwrap()
    .unwrap()
    .binding;
    request.via = vec![
        BusVia {
            id: "first".into(),
            tile_key: tile.canonical_url.clone(),
            vertex: bus_vertex(0, 2),
        },
        BusVia {
            id: "second".into(),
            tile_key: tile.canonical_url.clone(),
            vertex: bus_vertex(0, 1),
        },
    ];
    assert!(reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .is_none());
    request.respect_oneway = false;
    assert!(reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .is_some());
}

type GradeFixtureLine<'a> = (&'a [[i32; 2]], &'a [(&'a str, &'a str)]);
fn grade_tile(lines: &[GradeFixtureLine<'_>]) -> Vec<u8> {
    let mut keys = vec![];
    let mut values = vec![];
    let mut fs = vec![];
    for &(points, attrs) in lines {
        let tags: Vec<_> = attrs
            .iter()
            .flat_map(|&(k, v)| {
                let i = keys.iter().position(|x| x == &k).unwrap_or_else(|| {
                    keys.push(k);
                    keys.len() - 1
                });
                let j = values.len();
                values.push(field(1, v.as_bytes()));
                [i as u64, j as u64]
            })
            .collect();
        fs.push(feature(&tags, &bus_geometry(points)));
    }
    field(3, &layer(&fs, &keys, &values))
}
fn observed_query(tile: &TileBinding, start: BusVertex, end: BusVertex) -> BusPathBinding {
    let mut query = bus_rectangle_query(std::slice::from_ref(tile), end, false);
    query.rule = BUS_GRAPH_RULE.into();
    query.end_tile_key = None;
    query.grade_policy = Some(BusGradePolicy::ObservedConnectors);
    query.start = start;
    query
}

#[test]
fn ordinary_bus_bridges_and_tunnels_have_exact_approaches_but_not_underpass_turns() {
    let common = &[("class", "minor")];
    for (kind, lvl) in [("bridge", "1"), ("tunnel", "-1")] {
        let elevated = &[("class", "minor"), ("brunnel", kind), ("layer", lvl)];
        let raw = grade_tile(&[
            (&[[100, 100], [300, 100]], common),
            (&[[300, 100], [450, 100], [700, 100]], elevated),
            (&[[450, 40], [450, 100], [450, 200]], common),
            (&[[700, 100], [900, 100]], common),
        ]);
        let tile = binding(&raw);
        let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![tile.clone()]).unwrap();
        let q = observed_query(&tile, bus_vertex(0, 0), bus_vertex(3, 1));
        let works = reconstruct_gameplay_bus_path(&store, &q).unwrap().unwrap();
        assert_eq!(works.points.len(), 5);
        assert_eq!(works.spans.len(), 3);
        let mut wrong = q.clone();
        wrong.end = bus_vertex(2, 2);
        assert!(reconstruct_gameplay_bus_path(&store, &wrong)
            .unwrap()
            .is_none());
        wrong.start = bus_vertex(2, 0);
        wrong.end = bus_vertex(1, 1);
        assert!(reconstruct_gameplay_bus_path(&store, &wrong)
            .unwrap()
            .is_none());
        let mut old = q;
        old.grade_policy = None;
        assert!(!serde_json::to_string(&old).unwrap().contains("gradePolicy"));
        assert!(reconstruct_gameplay_bus_path(&store, &old)
            .unwrap()
            .is_none());
        old.end = bus_vertex(0, 1);
        let old_ground = reconstruct_gameplay_bus_path(&store, &old)
            .unwrap()
            .unwrap();
        old.grade_policy = Some(BusGradePolicy::ObservedConnectors);
        let new_ground = reconstruct_gameplay_bus_path(&store, &old)
            .unwrap()
            .unwrap();
        assert_eq!(old_ground.points, new_ground.points);
        assert_eq!(old_ground.source_atom_id(0), new_ground.source_atom_id(0));
    }
}

#[test]
fn ordinary_bus_grade_changes_refuse_ambiguity_wrong_angle_and_denied_access() {
    let common = &[("class", "minor")];
    let elevated = &[("class", "minor"), ("brunnel", "bridge"), ("layer", "1")];
    let private = &[("class", "minor"), ("access", "private")];
    let denied = &[
        ("class", "minor"),
        ("brunnel", "bridge"),
        ("layer", "1"),
        ("access", "no"),
    ];
    let scenarios: Vec<Vec<GradeFixtureLine<'_>>> = vec![
        vec![
            (&[[100, 100], [300, 100]], common),
            (&[[300, 100], [700, 100]], elevated),
            (&[[300, 100], [300, 300]], private),
        ],
        vec![
            (&[[100, 100], [300, 100]], common),
            (&[[300, 100], [300, 500]], elevated),
        ],
        vec![
            (&[[100, 100], [300, 100]], common),
            (&[[300, 100], [700, 100]], denied),
        ],
    ];
    for features in scenarios {
        let raw = grade_tile(&features);
        let tile = binding(&raw);
        let store = EvidenceStore::from_parts(vec![raw_entry(&raw)], vec![tile.clone()]).unwrap();
        let q = observed_query(&tile, bus_vertex(0, 0), bus_vertex(1, 1));
        assert!(reconstruct_gameplay_bus_path(&store, &q).unwrap().is_none());
    }
}

#[test]
fn ordinary_bus_same_grade_motorway_seam_carries_connected_approaches_only() {
    let ground = &[("class", "motorway")];
    let bridge = &[("class", "motorway"), ("brunnel", "bridge"), ("layer", "1")];
    let tunnel = &[
        ("class", "motorway"),
        ("brunnel", "tunnel"),
        ("layer", "-1"),
    ];
    let wrong_layer = &[("class", "motorway"), ("brunnel", "bridge"), ("layer", "2")];
    for (end_grade, pass) in [(bridge, true), (tunnel, false), (wrong_layer, false)] {
        let west = grade_tile(&[
            (&[[3000, 2010], [3500, 2010]], ground),
            (&[[3500, 2010], [4200, 2010]], bridge),
        ]);
        let east = grade_tile(&[
            (&[[-104, 2010], [700, 2010]], end_grade),
            (&[[700, 2010], [900, 2010]], ground),
        ]);
        let mut a = binding(&west);
        let mut b = binding(&east);
        a.x = 14552;
        a.canonical_url = a.canonical_url.replace("/14553/", "/14552/");
        b.x = 14553;
        let tiles = vec![a, b];
        let store = EvidenceStore::from_archive_parts(
            vec![raw_entry(&west), raw_entry(&east)],
            tiles.clone(),
        )
        .unwrap();
        let mut q = bus_rectangle_query(&tiles, bus_vertex(1, 1), false);
        q.grade_policy = Some(BusGradePolicy::ObservedConnectors);
        let path = reconstruct_gameplay_bus_path(&store, &q).unwrap();
        assert_eq!(path.is_some(), pass);
        if let Some(path) = path {
            assert_eq!(path.spans.len(), 4);
        }
        q.grade_policy = None;
        assert!(reconstruct_gameplay_bus_path(&store, &q).unwrap().is_none());
    }
}

#[test]
fn authored_tram_uses_exact_ground_junction_without_treating_motor_direction_as_rail_authority() {
    let t = |road: Option<(&str, &str)>| {
        bus_fixture(&[
            (
                &[[100, 100], [200, 100], [300, 100]],
                Some(("oneway", "yes")),
            ),
            (&[[200, 100], [200, 200]], road),
        ])
    };
    let (b, store) = t(None);
    let guide = |binding: &TileBinding, start, end| AuthoredTramGuide {
        adjacent_end: None,
        start_fraction_q: None,
        end_fraction_q: None,
        tile_key: binding.canonical_url.clone(),
        tile_raw_sha256: binding.raw_sha256.clone(),
        start,
        end,
    };
    let start = bus_vertex(0, 0);
    let finish = bus_vertex(1, 1);
    let a = reconstruct_authored_tram_guide(&store, &guide(&b, start, finish))
        .unwrap()
        .unwrap();
    assert_eq!(a.len(), 3);
    let back = reconstruct_authored_tram_guide(&store, &guide(&b, finish, start))
        .unwrap()
        .unwrap();
    assert!(a.iter().rev().eq(&back));
    let fractional = AuthoredTramGuide {
        start_fraction_q: Some(250_000),
        end_fraction_q: Some(750_000),
        ..guide(&b, start, bus_vertex(1, 0))
    };
    let partial = reconstruct_authored_tram_guide(&store, &fractional)
        .unwrap()
        .unwrap();
    assert_eq!(partial.len(), 3);
    assert_eq!(partial[0][0], a[0][0] + (a[1][0] - a[0][0]) * 0.25);
    assert_eq!(partial[1], a[1]);
    assert_eq!(partial[2][1], a[1][1] + (a[2][1] - a[1][1]) * 0.75);
    let same_atom = AuthoredTramGuide {
        start_fraction_q: Some(250_000),
        end_fraction_q: Some(500_000),
        ..guide(&b, start, start)
    };
    let direct = reconstruct_authored_tram_guide(&store, &same_atom)
        .unwrap()
        .unwrap();
    assert_eq!(direct.len(), 2);
    assert_eq!(direct[0], partial[0]);
    assert_eq!(direct[1][0], a[0][0] + (a[1][0] - a[0][0]) * 0.5);
    let refused = AuthoredTramGuide {
        start_fraction_q: Some(1_000_001),
        ..fractional
    };
    assert!(reconstruct_authored_tram_guide(&store, &refused)
        .unwrap()
        .is_none());
    assert!(
        find_gameplay_bus_path(&store, &b.canonical_url, finish, start, true)
            .unwrap()
            .is_none()
    );
    let mut tamper = guide(&b, start, finish);
    tamper.tile_raw_sha256 = "0".repeat(64);
    assert_eq!(
        reconstruct_authored_tram_guide(&store, &tamper),
        Err(Error::Hash)
    );
    for attr in [
        ("access", "private"),
        ("access", "unknown"),
        ("brunnel", "bridge"),
        ("layer", "1"),
    ] {
        let (b, scoped) = t(Some(attr));
        assert!(
            reconstruct_authored_tram_guide(&scoped, &guide(&b, start, finish))
                .unwrap()
                .is_none()
        );
    }
    let (b, scoped) = bus_fixture(&[
        (&[[100, 100], [200, 100], [300, 100]], None),
        (&[[200, 50], [200, 100], [200, 150]], None),
    ]);
    assert!(
        reconstruct_authored_tram_guide(&scoped, &guide(&b, start, bus_vertex(1, 2)))
            .unwrap()
            .is_none()
    ); // overlapping interior-only vertices do not join
    let (b, scoped) = bus_fixture(&[
        (&[[100, 100], [200, 100], [300, 100]], None),
        (&[[200, 100], [200, 200]], None),
        (&[[200, 100], [220, 25]], Some(("access", "private"))),
    ]);
    assert!(
        reconstruct_authored_tram_guide(&scoped, &guide(&b, start, finish))
            .unwrap()
            .is_none()
    ); // unknown/denied third arm is not ignored
}

#[test]
fn ordinary_bus_markers_follow_both_verified_return_intervals_at_an_interior_stop() {
    use rosenfu_core::ofm_evidence::{BusInteriorStop, BusPattern};
    use rosenfu_core::operation::{Timetable, Vehicle};
    use rosenfu_core::world::{BusOperationConfig, GeoPoint, World};

    let cells: &[(u32, u32, &[BusFixtureLine<'_>])] = &[
        (14552, 6451, &[(&[[3200, 2010], [4200, 2010]], None)]),
        (14553, 6451, &[(&[[-104, 2010], [4200, 2010]], None)]),
        (14554, 6451, &[(&[[-104, 2010], [896, 2010]], None)]),
    ];
    let (tiles, store) = bus_rectangle(cells);
    let mut request = bus_rectangle_query(&tiles, bus_vertex(0, 1), false);
    request.pattern = Some(BusPattern::OpenRoundTrip);
    let outbound = reconstruct_gameplay_bus_path(&store, &request)
        .unwrap()
        .unwrap();
    let keys = outbound.stop_occurrence_keys();
    assert_eq!(keys.len(), 3);
    request.interior_stops.push(BusInteriorStop {
        id: "middle".into(),
        name: "途中".into(),
        occurrence_key: keys[1].clone(),
        fraction_q: 370_000,
    });
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let owner = world.add_company(player, "Bus".into()).unwrap();
    let evidence = cells
        .iter()
        .zip(tiles)
        .map(|((_, _, lines), tile)| (raw_entry(&bus_fixture_raw(lines)), tile))
        .collect();
    let mut world = world
        .candidate_with_gameplay_ofm_bus(
            owner,
            "test".into(),
            "往復".into(),
            request.clone(),
            evidence,
        )
        .unwrap();
    let returned = world
        .snapshot()
        .gameplay_bus_return_path(&request)
        .unwrap()
        .unwrap();
    // The ordinary stop editor uses this same native candidate, keeping
    // the exact stop identity while changing only its location on the route.
    let original = world
        .snapshot()
        .gameplay_bus_interior_stop_views("test", &request, Some(&returned))
        .unwrap();
    let mut relocated = request.clone();
    relocated.interior_stops[0].fraction_q = 570_000;
    let relocated_world = world
        .candidate_with_gameplay_ofm_bus(
            owner,
            "test".into(),
            "往復".into(),
            relocated.clone(),
            vec![], // no new source: the saved raw evidence is sufficient
        )
        .unwrap();
    let relocated_stops = relocated_world
        .snapshot()
        .gameplay_bus_interior_stop_views("test", &relocated, Some(&returned))
        .unwrap();
    assert_eq!(relocated_stops[0].id, original[0].id);
    assert_eq!(relocated_stops[0].stop_id, original[0].stop_id);
    assert_eq!(relocated_stops[0].name, original[0].name);
    assert!(relocated_stops[0].serves_return);
    assert_ne!(relocated_stops[0].point, original[0].point);
    assert_eq!(
        world.snapshot().ofm_gameplay_bus_routes["test"].interior_stops[0].fraction_q,
        370_000
    );

    let config = BusOperationConfig {
        route_id: "test".into(),
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 0,
            headway_ms: 60_000,
        },
        vehicle: Vehicle {
            max_speed_kmh: 72.,
            acceleration_mps2: 1.,
            braking_mps2: 1.,
            length_m: 12.,
        },
        assumed_speed_kmh: 72.,
        dwell_ms: 10_000,
        turnaround_ms: None,
    };
    world
        .configure_all_operation(vec![], vec![config.clone()])
        .unwrap();

    let operation = world.operation().unwrap();
    let spec = operation.runtime.service_spec("bus:test").unwrap();
    assert_eq!(spec.legs.len(), 4); // two outward and two independently proved return intervals
    assert_eq!(spec.legs[2].to, spec.legs[3].from);
    assert_eq!(spec.legs[1].to, spec.legs[2].from);
    assert_eq!(spec.legs[3].to, spec.legs[0].from);
    assert_eq!(spec.legs[0].to, spec.legs[2].to);
    assert_eq!(spec.legs[2].sections[0].resources.len(), 2);
    assert_eq!(spec.legs[3].sections[0].resources.len(), 2);
    let observed = operation
        .runtime
        .preview_passenger_replay(900_000, 128)
        .unwrap();
    assert!(observed.complete);
    assert_eq!(observed.completed_legs.len(), 4);

    let advance = |world: &mut World, at| {
        let result = world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(at, 128)
            .unwrap();
        assert!(result.reached_target);
    };
    let point = |i: usize| {
        let [longitude, latitude] = returned.points[i];
        GeoPoint {
            latitude,
            longitude,
        }
    };
    for (sequence, atom, initial_position, end_position) in [(2, 0, 0., 0.63), (3, 1, 0.63, 1.)] {
        let leg = &observed.completed_legs[sequence];
        assert_eq!(leg.sequence, sequence);
        advance(&mut world, leg.entered_ms + 1_000);
        let committed = world.operation_save();
        let markers = world.live_straight_bus_markers();
        assert_eq!(markers.len(), 1, "return interval {sequence}");
        let marker = &markers[0];
        assert_eq!(marker.key, leg.train);
        assert_eq!(marker.edge_start, point(atom));
        assert_eq!(marker.edge_end, point(atom + 1));
        assert!(marker.position > initial_position && marker.position < initial_position + 0.01);
        assert_eq!(world.operation_save(), committed);

        advance(&mut world, leg.arrived_ms - 1_000);
        let traversed = world.live_straight_bus_markers();
        assert_eq!(traversed.len(), 1);
        assert_eq!(traversed[0].edge_start, point(atom + 1));
        assert_eq!(traversed[0].edge_end, point(atom + 2));
        assert!(
            traversed[0].position < end_position && traversed[0].position > end_position - 0.01
        );

        advance(&mut world, leg.arrived_ms + 1);
        assert!(world.live_straight_bus_markers().is_empty());
    }

    request.interior_stops.clear();
    let mut direct = world
        .candidate_with_gameplay_ofm_bus(owner, "test".into(), "直接往復".into(), request, vec![])
        .unwrap();
    direct
        .configure_all_operation(vec![], vec![config])
        .unwrap();
    let observed = direct
        .operation()
        .unwrap()
        .runtime
        .preview_passenger_replay(900_000, 128)
        .unwrap();
    assert!(observed.complete);
    assert_eq!(observed.completed_legs.len(), 2);
    advance(&mut direct, observed.completed_legs[1].entered_ms + 1_000);
    let direct_markers = direct.live_straight_bus_markers();
    assert_eq!(direct_markers.len(), 1);
    assert_eq!(direct_markers[0].edge_start, point(0));
    assert_eq!(direct_markers[0].edge_end, point(1));
}

#[test]
fn ordinary_street_tram_adjacent_seam_rejects_wrong_hash_and_buffer_pick() {
    use rosenfu_core::ofm_evidence::BusTileProof;
    let left: &[BusFixtureLine<'_>] = &[(&[[3200, 2010], [4200, 2010]], Some(("oneway", "yes")))];
    let test = |right: &[BusFixtureLine<'_>], right_x, right_y| {
        let (tiles, store) = bus_rectangle(&[(14552, 6451, left), (right_x, right_y, right)]);
        let guide = AuthoredTramGuide {
            adjacent_end: Some(BusTileProof {
                tile_key: tiles[1].canonical_url.clone(),
                tile_raw_sha256: tiles[1].raw_sha256.clone(),
            }),
            tile_key: tiles[0].canonical_url.clone(),
            tile_raw_sha256: tiles[0].raw_sha256.clone(),
            start: bus_vertex(0, 0),
            end: bus_vertex(0, 0),
            start_fraction_q: Some(500_000),
            end_fraction_q: Some(500_000),
        };
        (tiles, store, guide)
    };
    let right: &[BusFixtureLine<'_>] = &[(&[[-104, 2010], [896, 2010]], Some(("oneway", "yes")))];
    let (tiles, store, guide) = test(right, 14553, 6451);
    let route = reconstruct_authored_tram_guide(&store, &guide)
        .unwrap()
        .unwrap();
    let cutoff = f64::from(14553_u32) / 16384. * 360. - 180.;
    assert_eq!(route.len(), 3);
    assert!(route[0][0] < cutoff && route[2][0] > cutoff);
    assert_eq!(route[1][0], cutoff);

    let reverse = AuthoredTramGuide {
        adjacent_end: Some(BusTileProof {
            tile_key: tiles[0].canonical_url.clone(),
            tile_raw_sha256: tiles[0].raw_sha256.clone(),
        }),
        tile_key: tiles[1].canonical_url.clone(),
        tile_raw_sha256: tiles[1].raw_sha256.clone(),
        ..guide.clone()
    };
    let reversed = reconstruct_authored_tram_guide(&store, &reverse)
        .unwrap()
        .unwrap();
    assert!(reversed.iter().rev().eq(route.iter()));
    let mut changed = guide.clone();
    changed.adjacent_end.as_mut().unwrap().tile_raw_sha256 = "0".repeat(64);
    assert_eq!(
        reconstruct_authored_tram_guide(&store, &changed),
        Err(Error::Hash)
    );
    let mut buffer = guide;
    buffer.start_fraction_q = Some(950_000);
    assert!(reconstruct_authored_tram_guide(&store, &buffer)
        .unwrap()
        .is_none());

    let (_, diagonal, diagonal_guide) = test(right, 14553, 6452);
    assert_eq!(
        reconstruct_authored_tram_guide(&diagonal, &diagonal_guide),
        Err(Error::Binding)
    );
    let (_, disconnected, disconnected_guide) = test(
        &[(&[[-104, 2011], [896, 2011]], Some(("oneway", "yes")))],
        14553,
        6451,
    );
    assert!(
        reconstruct_authored_tram_guide(&disconnected, &disconnected_guide)
            .unwrap()
            .is_none()
    );
    let ambiguous = [right[0], right[0]];
    let (_, store, guide) = test(&ambiguous, 14553, 6451);
    assert!(reconstruct_authored_tram_guide(&store, &guide)
        .unwrap()
        .is_none());
}

#[test]
fn authored_tram_chain_two_north_south_seams_require_exact_order_and_evidence() {
    use rosenfu_core::ofm_evidence::{
        reconstruct_authored_tram_chain, AuthoredTramChainGuide, BusTileProof,
    };
    let north: &[BusFixtureLine<'_>] = &[(&[[2000, 3400], [2000, 4196]], None)];
    let middle: &[BusFixtureLine<'_>] = &[(
        &[[2000, -100], [2000, 1200], [2000, 2900], [2000, 4196]],
        None,
    )];
    let south: &[BusFixtureLine<'_>] = &[(&[[2000, -100], [2000, 600]], None)];
    let cells: [(u32, u32, &[BusFixtureLine<'_>]); 3] = [
        (14391, 6428, north),
        (14391, 6429, middle),
        (14391, 6430, south),
    ];
    let make = |bindings: &[TileBinding]| AuthoredTramChainGuide {
        tiles: bindings
            .iter()
            .map(|t| BusTileProof {
                tile_key: t.canonical_url.clone(),
                tile_raw_sha256: t.raw_sha256.clone(),
            })
            .collect(),
        start: bus_vertex(0, 0),
        end: bus_vertex(0, 0),
        start_fraction_q: Some(500_000),
        end_fraction_q: Some(500_000),
    };
    let (tiles, store) = bus_rectangle(&cells);
    let guide = make(&tiles);
    let path = reconstruct_authored_tram_chain(&store, &guide)
        .unwrap()
        .unwrap();
    let latitude = |y: f64| {
        (std::f64::consts::PI * (1. - 2. * y / 16384.))
            .sinh()
            .atan()
            .to_degrees()
    };
    assert!(path.windows(2).all(|p| p[0][1] > p[1][1]));
    for seam in [6429., 6430.] {
        let cut = latitude(seam);
        assert_eq!(path.iter().filter(|p| (p[1] - cut).abs() < 1e-8).count(), 1);
    }
    let mut reverse = make(&tiles);
    reverse.tiles.reverse();
    assert!(reconstruct_authored_tram_chain(&store, &reverse)
        .unwrap()
        .unwrap()
        .iter()
        .rev()
        .eq(path.iter()));

    let mut outside = make(&tiles);
    outside.start_fraction_q = Some(950_000);
    assert!(reconstruct_authored_tram_chain(&store, &outside)
        .unwrap()
        .is_none());
    let mut digest = make(&tiles);
    digest.tiles[1].tile_raw_sha256 = "0".repeat(64);
    assert_eq!(
        reconstruct_authored_tram_chain(&store, &digest),
        Err(Error::Hash)
    );
    let mut repeat = make(&tiles);
    repeat.tiles[1] = repeat.tiles[0].clone();
    assert_eq!(
        reconstruct_authored_tram_chain(&store, &repeat),
        Err(Error::Binding)
    );
    let mut skip = make(&tiles);
    skip.tiles.swap(1, 2);
    assert_eq!(
        reconstruct_authored_tram_chain(&store, &skip),
        Err(Error::Binding)
    );

    let doubled = [middle[0], middle[0]];
    let (tiles, ambiguous) = bus_rectangle(&[
        (14391, 6428, north),
        (14391, 6429, &doubled),
        (14391, 6430, south),
    ]);
    assert!(reconstruct_authored_tram_chain(&ambiguous, &make(&tiles))
        .unwrap()
        .is_none());
    let (tiles, corner) = bus_rectangle(&[
        (14391, 6428, north),
        (14392, 6429, middle),
        (14392, 6430, south),
    ]);
    assert_eq!(
        reconstruct_authored_tram_chain(&corner, &make(&tiles)),
        Err(Error::Binding)
    );
}

#[test]
fn authored_tram_chain_can_turn_through_an_observed_middle_tile_not_a_rectangle() {
    use rosenfu_core::ofm_evidence::{
        reconstruct_authored_tram_chain, AuthoredTramChainGuide, BusTileProof,
    };
    let north: &[BusFixtureLine<'_>] = &[(&[[2000, 3400], [2000, 4196]], None)];
    let corner: &[BusFixtureLine<'_>] = &[(&[[2000, -100], [2000, 2100], [4196, 2100]], None)];
    let east: &[BusFixtureLine<'_>] = &[(&[[-100, 2100], [700, 2100]], None)];
    let (tiles, store) = bus_rectangle(&[
        (14391, 6428, north),
        (14391, 6429, corner),
        (14392, 6429, east),
    ]);
    let guide = AuthoredTramChainGuide {
        tiles: tiles
            .iter()
            .map(|t| BusTileProof {
                tile_key: t.canonical_url.clone(),
                tile_raw_sha256: t.raw_sha256.clone(),
            })
            .collect(),
        start: bus_vertex(0, 0),
        end: bus_vertex(0, 1),
        start_fraction_q: None,
        end_fraction_q: None,
    };
    let path = reconstruct_authored_tram_chain(&store, &guide)
        .unwrap()
        .unwrap();
    assert_eq!(path.len(), 5);
    assert!(path[0][0] == path[1][0] && path[1][0] == path[2][0]);
    assert!(path[0][1] > path[1][1] && path[1][1] > path[2][1]);
    assert!(path[2][0] < path[3][0] && path[3][0] < path[4][0]);
    assert!(path[2..].iter().all(|p| p[1] == path[2][1]));
}

#[test]
fn fractional_bus_traversal_preserves_regular_atom_ids_and_exact_terminal_positions() {
    let (tile, store) = bus_fixture(&[
        (&[[100, 100], [300, 100], [500, 100]], None),
        (&[[500, 100], [500, 300]], None),
    ]);
    let anchor = |feature| BusVertex {
        feature,
        line: 0,
        vertex: 0,
    };
    let request = BusPathBinding {
        tile_key: tile.canonical_url.clone(),
        tile_raw_sha256: tile.raw_sha256.clone(),
        start: anchor(0),
        end: anchor(1),
        rule: BUS_GRAPH_RULE.into(),
        respect_oneway: true,
        same_atom_stops: None,
        fractional_terminals: None,
        grade_policy: None,
        additional_tiles: vec![],
        end_tile_key: None,
        pattern: None,
        interior_stops: vec![],
        via: vec![],
    };
    let route = resolve_fractional_bus_traversal(&store, &request, [250_000, 750_000])
        .unwrap()
        .unwrap();
    assert_eq!(route.atom_count(), 3);
    let vertex_route = find_gameplay_bus_path(
        &store,
        &tile.canonical_url,
        anchor(0),
        bus_vertex(1, 1),
        true,
    )
    .unwrap()
    .unwrap();
    for i in 0..route.atom_count() {
        assert_eq!(route.directed_atom_id(i), vertex_route.source_atom_id(i));
    }
    assert_eq!(route.terminal_cuts(), [250_000, 750_000]);
    let points = route.oriented_route_points();
    assert_eq!(points[1..3], vertex_route.points[1..3]);
    assert!(points[0][0] > vertex_route.points[0][0]);
    assert!(points[3][1] > vertex_route.points[3][1]);
    assert!(route.length_mm() > 1);
    assert!((route.length_mm() as f64) < vertex_route.length_m * 1000.);

    let reverse = BusPathBinding {
        start: request.end,
        end: request.start,
        ..request.clone()
    };
    let back = resolve_fractional_bus_traversal(&store, &reverse, [750_000, 250_000])
        .unwrap()
        .unwrap();
    assert_eq!(back.atom_count(), 3);
    assert!((0..3).all(|i| back.physical_atom_id(i) == route.physical_atom_id(2 - i)));
    assert_ne!(back.directed_atom_id(0), route.directed_atom_id(2));
    for (a, b) in points.iter().zip(back.oriented_route_points().iter().rev()) {
        assert!((a[0] - b[0]).abs() < 1e-10 && (a[1] - b[1]).abs() < 1e-10);
    }

    let one_way = bus_fixture(&[
        (&[[100, 100], [300, 100], [500, 100]], None),
        (&[[500, 100], [500, 300]], Some(("oneway", "yes"))),
    ]);
    let denied = BusPathBinding {
        tile_key: one_way.0.canonical_url.clone(),
        tile_raw_sha256: one_way.0.raw_sha256.clone(),
        ..reverse
    };
    assert!(
        resolve_fractional_bus_traversal(&one_way.1, &denied, [750_000, 250_000])
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        resolve_fractional_bus_traversal(&one_way.1, &request, [250_000, 750_000]),
        Err(Error::Hash)
    ));
    assert!(matches!(
        resolve_fractional_bus_traversal(&store, &request, [0, 750_000]),
        Err(Error::Binding)
    ));
}

#[test]
fn raw_feature_ids_are_opaque_and_only_unique_nonzero_singletons_resolve() {
    fn with_id(id: u64, geometry: &[u64]) -> Vec<u8> {
        [scalar(1, id), feature(&[0, 0], geometry)].concat()
    }
    let raw = field(
        3,
        &layer(
            &[
                with_id(u64::MAX, LINE),
                feature(&[0, 0], LINE),
                with_id(0, LINE),
                with_id(19, LINE),
                with_id(19, LINE),
                with_id(32, LINE),
                [scalar(1, 32), scalar(3, 1)].concat(),
                with_id(71, &[9, 0, 0, 10, 20, 0, 9, 1, 1, 10, 2, 2]),
            ],
            &["class"],
            &[field(1, b"minor")],
        ),
    );
    let decoded = decode_tile(&binding(&raw), &raw).unwrap();
    assert_eq!(
        decoded
            .unique_raw_road_line(u64::MAX)
            .unwrap()
            .feature_index,
        0
    );
    assert_eq!(decoded.lines[1].raw_feature_id, None);
    assert_eq!(decoded.lines[2].raw_feature_id, Some(0));
    for id in [0, 19, 32, 71, 999] {
        assert!(decoded.unique_raw_road_line(id).is_none());
    }
}

#[test]
fn opt_in_westbound_seam_verifies_exact_shared_atom_and_rejects_ambiguity() {
    let west = [[4176, 1800], [4156, 1790], [3856, 1640]];
    let east = [[180, 1850], [80, 1800], [60, 1790], [-1385, 1067]];
    let make = |feature_id, points: &[[i32; 2]]| {
        [
            scalar(1, feature_id),
            feature(&[0, 0, 1, 1], &xy_geometry(points)),
        ]
        .concat()
    };
    let raw = |features: &[Vec<u8>]| {
        field(
            3,
            &layer(
                features,
                &["class", "oneway"],
                &[field(1, b"secondary"), scalar(5, 1)],
            ),
        )
    };
    let setup = |contested| {
        let a = raw(&[make(21, &west)]);
        let mut b_features = vec![make(21, &east), make(99, &[[500, 500], [450, 500]])];
        if contested {
            b_features.push(make(22, &east));
        }
        let b = raw(&b_features);
        let mut west = binding(&a);
        west.x -= 1;
        west.canonical_url = west.canonical_url.replace("14553", "14552");
        let tiles = [west, binding(&b)];
        let store =
            EvidenceStore::from_parts(vec![raw_entry(&a), raw_entry(&b)], tiles.to_vec()).unwrap();
        let receipt = WestboundSeamReceipt {
            rule: WESTBOUND_SEAM_WITNESS_RULE.to_owned(),
            tiles_west_east: tiles,
            raw_feature_id: 21,
            vertices_west_east: [1, 2],
        };
        (store, receipt)
    };
    let verify = |store, receipt: &WestboundSeamReceipt| {
        let bytes = serde_json::to_vec(receipt).unwrap();
        verify_westbound_seam_receipt(store, &bytes, &raw_sha256(&bytes))
    };
    let (store, receipt) = setup(false);
    assert_eq!(
        verify(&store, &receipt).unwrap().shared_atom,
        [[80, 1800], [60, 1790]]
    );
    let retained = EvidenceStore::from_archive_parts(
        store
            .blobs()
            .map(|(sha, bytes)| RawEvidence {
                sha256: sha.to_owned(),
                bytes: bytes.to_vec(),
            })
            .collect(),
        receipt.tiles_west_east.to_vec(),
    )
    .unwrap();
    let rebuilt = retained
        .scope(&receipt.tiles_west_east.clone().map(|t| t.canonical_url))
        .unwrap();
    assert_eq!(verify(&rebuilt, &receipt), verify(&store, &receipt));
    let bytes = serde_json::to_vec(&receipt).unwrap();
    assert_eq!(
        verify_westbound_seam_receipt(&store, &bytes, &raw_sha256(b"other")),
        Err(Error::Hash)
    );
    let digest = raw_sha256(&bytes);
    let traversal = replay_westbound_seam_traversal(&rebuilt, &bytes, &digest).unwrap();
    assert_eq!(
        traversal.grid_points(),
        &[[80, 1800], [60, 1790], [-240, 1640]]
    );
    assert_eq!(
        traversal
            .atoms_in_order()
            .each_ref()
            .map(|atom| atom.raw_ordinal),
        [1, 1]
    );
    assert_eq!(
        traversal.atoms_in_order()[0].tile,
        receipt.tiles_west_east[1]
    );
    assert_eq!(
        traversal.atoms_in_order()[1].tile,
        receipt.tiles_west_east[0]
    );
    let east = &receipt.tiles_west_east[1].canonical_url;
    let east_scope = rebuilt.scope(std::slice::from_ref(east)).unwrap();
    let route = |feature| {
        find_gameplay_bus_path(
            &east_scope,
            east,
            bus_vertex(feature, 0),
            bus_vertex(feature, 1),
            true,
        )
        .unwrap()
        .unwrap()
    };
    // The east side's preceding atom isn't in the traversal, but a legacy
    // vehicle on that same source fragment still conservatively conflicts.
    assert!(traversal.overlaps_legacy_fragments(&route(0)));
    assert!(!traversal.overlaps_legacy_fragments(&route(1)));
    let mut wrong = receipt.clone();
    wrong.vertices_west_east = [0, 2];
    assert_eq!(verify(&store, &wrong), Err(Error::Binding));
    wrong = receipt;
    wrong.tiles_west_east[1].raw_sha256 = raw_sha256(b"other");
    assert_eq!(verify(&store, &wrong), Err(Error::Binding));
    let (contested, receipt) = setup(true);
    assert_eq!(verify(&contested, &receipt), Err(Error::Binding));
}
