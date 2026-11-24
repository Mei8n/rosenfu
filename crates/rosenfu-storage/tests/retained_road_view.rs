//! Ordinary disk save/reopen followed by the native display-only read.
use rosenfu_core::{
    ofm_evidence::{
        raw_sha256, retained_road_view, EvidenceStore, RawEvidence, TileBinding, DECODER_RULE,
    },
    world::{World, WorldDraft},
};
use rosenfu_storage::{file_version, load_world_with_version, save_world};

#[test]
fn ordinary_world_reopen_can_display_each_retained_region_offline() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../rosenfu-core/tests/fixtures/ofm_session_synthetic.json"
    ))
    .unwrap();
    let bytes: Vec<u8> =
        serde_json::from_value(fixture["rawEvidence"][0]["bytes"].clone()).unwrap();
    let sha = raw_sha256(&bytes);
    let rows = [(14552, 6451), (14624, 6016)]
        .into_iter()
        .map(|(x, y)| TileBinding {
            canonical_url: format!("https://tiles.openfreemap.org/planet/test/14/{x}/{y}.pbf"),
            revision: "test".into(),
            decoder_rule: DECODER_RULE.into(),
            raw_sha256: sha.clone(),
            z: 14,
            x,
            y,
        })
        .collect();
    let store =
        EvidenceStore::from_archive_parts(vec![RawEvidence { sha256: sha, bytes }], rows).unwrap();
    let world = World::new(WorldDraft::default().with_road_evidence(store).unwrap()).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("distant-regions.rosenfu");
    save_world(&file, &world).unwrap();
    let original = std::fs::read(&file).unwrap();
    drop(world);
    let (reopened, version) = load_world_with_version(&file).unwrap();
    let archive = &reopened.snapshot().runtime_road_evidence;
    let near = retained_road_view(archive, [14552, 6451, 14552, 6451]).unwrap();
    let far = retained_road_view(archive, [14624, 6016, 14624, 6016]).unwrap();
    assert_eq!((near.requested_tiles, near.retained_tiles), (1, 1));
    assert_eq!((far.requested_tiles, far.retained_tiles), (1, 1));
    assert!(!near.segments.is_empty() && !far.segments.is_empty());
    assert_eq!(near.segments.len(), far.segments.len());
    assert_ne!(near.segments, far.segments);
    assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), original);
    assert_eq!(file_version(&file).unwrap(), version);
}
