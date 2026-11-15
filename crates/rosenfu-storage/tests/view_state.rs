use rosenfu_core::world::{ViewState, World, WorldDraft};
use rosenfu_storage::{decode, encode};
#[test]
fn view_roundtrips_and_absent_optional_view_remains_absent() {
    let world = World::new(WorldDraft {
        view_state: Some(ViewState {
            latitude: 35.7,
            longitude: 139.8,
            zoom: 17.25,
        }),
        ..WorldDraft::default()
    })
    .unwrap();
    assert_eq!(decode(&encode(&world).unwrap()).unwrap(), world);
    let mut value: serde_json::Value = serde_json::from_slice(&encode(&world).unwrap()).unwrap();
    value["world"].as_object_mut().unwrap().remove("view_state");
    assert!(decode(&serde_json::to_vec(&value).unwrap())
        .unwrap()
        .snapshot()
        .view_state
        .is_none());
}
#[test]
fn invalid_view_is_rejected_without_changing_geometry() {
    for view in [
        ViewState {
            latitude: f64::NAN,
            longitude: 139.0,
            zoom: 12.0,
        },
        ViewState {
            latitude: 35.0,
            longitude: 155.0,
            zoom: 12.0,
        },
        ViewState {
            latitude: 35.0,
            longitude: 139.0,
            zoom: 21.0,
        },
    ] {
        assert!(World::new(WorldDraft {
            view_state: Some(view),
            ..WorldDraft::default()
        })
        .is_err());
    }
}
