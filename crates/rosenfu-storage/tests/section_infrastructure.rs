use rosenfu_core::world::*;
use rosenfu_storage::{decode, encode};

fn world() -> World {
    let mut world = World::default();
    let player = world.add_player("P".into()).unwrap();
    let company = world.add_company(player, "C".into()).unwrap();
    world
        .build_alignment(
            company,
            "A".into(),
            vec![
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.001,
                },
                GeoPoint {
                    latitude: 35.,
                    longitude: 139.002,
                },
            ],
        )
        .unwrap();
    world
}
#[test]
fn all_counts_and_seven_levels_roundtrip_without_changing_geometry_or_ids() {
    for count in [1, 2, 4] {
        for level in [
            InfrastructureLevel::Ground,
            InfrastructureLevel::Elevated,
            InfrastructureLevel::Elevated2,
            InfrastructureLevel::Elevated3,
            InfrastructureLevel::Underground,
            InfrastructureLevel::Underground2,
            InfrastructureLevel::Underground3,
        ] {
            let mut w = world();
            let original = w.snapshot().alignments[0].clone();
            let mut draft = w.snapshot().clone();
            let a = &mut draft.alignments[0];
            a.anchor_indices = Some(vec![0, 1, 2]);
            a.infrastructure = vec![
                SectionInfrastructure {
                    track_count: count,
                    level,
                },
                SectionInfrastructure {track_count:1,level},
            ];
            w.replace(draft).unwrap();
            let restored = decode(&encode(&w).unwrap()).unwrap();
            let a = &restored.snapshot().alignments[0];
            assert_eq!(a.id, original.id);
            assert_eq!(a.points, original.points);
            assert_eq!(
                a.section_infrastructure(0).unwrap(),
                SectionInfrastructure {
                    track_count: count,
                    level
                }
            );
            assert_eq!(
                a.section_infrastructure(1).unwrap(),
                SectionInfrastructure {track_count:1,level}
            );
            assert_eq!(a.section_infrastructure(2), None);
        }
    }
}
#[test]
fn invalid_count_and_section_shape_are_rejected_atomically() {
    let mut w = world();
    let before = w.clone();
    for infrastructure in [
        vec![SectionInfrastructure {
            track_count: 3,
            level: InfrastructureLevel::Ground,
        }],
        vec![SectionInfrastructure::default(); 2],
    ] {
        let mut draft = w.snapshot().clone();
        draft.alignments[0].infrastructure = infrastructure;
        assert!(w.replace(draft).is_err());
        assert_eq!(w, before);
    }
    let mut json: serde_json::Value = serde_json::from_slice(&encode(&w).unwrap()).unwrap();
    json["world"]["alignments"][0]["infrastructure"] =
        serde_json::json!([{"track_count":2,"level":"sky"}]);
    assert!(decode(&serde_json::to_vec(&json).unwrap()).is_err());
}
