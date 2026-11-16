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
