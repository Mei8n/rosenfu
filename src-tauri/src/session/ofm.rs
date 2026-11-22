//! Session-scoped evidence operations. Raw evidence never enters a UI snapshot.
use super::*;

use base64::Engine;
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfmRawInput {
    pub sha256: String,
    pub byte_length: usize,
    pub base64: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfmBindingInput {
    pub key: String,
    pub raw_sha256: String,
    pub url: String,
    pub revision: String,
    pub z: u8,
    pub x: u32,
    pub y: u32,
    pub decoder_rule: String,
    pub license: String,
    pub attribution: String,
}

pub(super) fn inputs(
    raw: Vec<OfmRawInput>,
    bindings: Vec<OfmBindingInput>,
) -> Result<
    (
        Vec<rosenfu_core::ofm_evidence::RawEvidence>,
        Vec<rosenfu_core::ofm_evidence::TileBinding>,
    ),
    String,
> {
    use rosenfu_core::ofm_evidence::{
        RawEvidence, TileBinding, MAX_REQUEST_BYTES, MAX_TILES, MAX_TILE_BYTES,
    };
    let err = || "道路の参照情報が不正または容量上限を超えています。".to_owned();
    if raw.len() > MAX_TILES || bindings.len() > MAX_TILES {
        return Err(err());
    }
    let mut total = 0usize;
    let mut decoded = Vec::new();
    for item in raw {
        if item.byte_length == 0
            || item.byte_length > MAX_TILE_BYTES
            || item.base64.len() != 4 * item.byte_length.div_ceil(3)
        {
            return Err(err());
        }
        total = total
            .checked_add(item.byte_length)
            .filter(|n| *n <= MAX_REQUEST_BYTES)
            .ok_or_else(err)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&item.base64)
            .map_err(|_| err())?;
        if bytes.len() != item.byte_length
            || base64::engine::general_purpose::STANDARD.encode(&bytes) != item.base64
        {
            return Err(err());
        }
        decoded.push(RawEvidence {
            sha256: item.sha256,
            bytes,
        });
    }
    let mut validated = Vec::new();
    for b in bindings {
        if b.key != b.url
            || b.license != "ODbL-1.0"
            || b.attribution != "© OpenMapTiles © OpenStreetMap contributors"
        {
            return Err(err());
        }
        validated.push(TileBinding {
            canonical_url: b.url,
            revision: b.revision,
            z: b.z,
            x: b.x,
            y: b.y,
            raw_sha256: b.raw_sha256,
            decoder_rule: b.decoder_rule,
        });
    }
    Ok((decoded, validated))
}

impl WorldEditor {
    /// Park a complete draft and its raw evidence together. Failure leaves world,
    /// IDs, deletion Undo, save destination and previously parked drafts unchanged.
    pub fn stage_ofm_construction_plan(
        &mut self,
        expected_token: &str,
        plans: rosenfu_core::world::ConstructionPlans,
        raw: Vec<OfmRawInput>,
        bindings: Vec<OfmBindingInput>,
    ) -> Result<WorldDraft, String> {
        if expected_token != self.ofm_world_token()? {
            return Err("ワールドが変更されたため道路計画の追加を中止しました。".into());
        }
        let (raw, bindings) = inputs(raw, bindings)?;
        let mut candidate = self.clone();
        candidate
            .world
            .stage_ofm_construction_plans(plans, raw, bindings)
            .map_err(|e| e.to_string())?;
        rosenfu_storage::ensure_world_saveable(&candidate.world).map_err(|e| e.to_string())?;
        self.commit_edit(candidate, false);
        Ok(self.snapshot())
    }

    /// Optimistic concurrency token for an evidence operation. It binds the
    /// complete current snapshot, including saved plans and evidence manifest,
    /// and the active destination. It is not an authentication credential.
    pub fn ofm_world_token(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(
            self.world.snapshot(),
            self.company,
            &self.active_slot_id,
            &self.loaded_path,
            self.company_setup_required,
        ))
        .map_err(|_| "ワールドの状態を確認できません。".to_owned())?;
        Ok(rosenfu_core::ofm_evidence::raw_sha256(&bytes))
    }

    /// UI previews receive descriptors but not raw evidence. Reattach only the
    /// current immutable table, rejecting changed or stale manifests.
    pub fn world_for_ui_preview(&self, draft: WorldDraft) -> Result<World, String> {
        self.world
            .replace_retaining_road_evidence(draft)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../crates/rosenfu-core/tests/fixtures/ofm_session_synthetic.json"
        ))
        .unwrap()
    }
    fn stage(editor: &mut WorldEditor) {
        stage_fixture(editor, fixture());
    }
    fn stage_fixture(editor: &mut WorldEditor, f: serde_json::Value) {
        let raw = f["rawEvidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let bytes: Vec<u8> = serde_json::from_value(r["bytes"].clone()).unwrap();
                OfmRawInput {
                    sha256: r["sha256"].as_str().unwrap().into(),
                    byte_length: bytes.len(),
                    base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                }
            })
            .collect();
        let bindings = f["tileBindings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| OfmBindingInput {
                key: r["canonical_url"].as_str().unwrap().into(),
                url: r["canonical_url"].as_str().unwrap().into(),
                raw_sha256: r["raw_sha256"].as_str().unwrap().into(),
                revision: r["revision"].as_str().unwrap().into(),
                z: 14,
                x: r["x"].as_u64().unwrap() as u32,
                y: r["y"].as_u64().unwrap() as u32,
                decoder_rule: r["decoder_rule"].as_str().unwrap().into(),
                license: "ODbL-1.0".into(),
                attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
            })
            .collect();
        editor
            .stage_ofm_construction_plan(
                &editor.ofm_world_token().unwrap(),
                serde_json::from_value(f["plans"].clone()).unwrap(),
                raw,
                bindings,
            )
            .unwrap();
    }
    #[test]
    fn ordinary_open_bus_returns_on_a_different_directed_path_without_minting_stops() {
        use rosenfu_core::ofm_evidence::{BusPathBinding, BusPattern, BusVertex, BUS_GRAPH_RULE};
        use rosenfu_core::operation::{Timetable, Vehicle};
        let mut editor = WorldEditor::new();
        editor.rename_company("往復のバス会社".into()).unwrap();
        let two_way = "GowBCg50cmFuc3BvcnRhdGlvbngCKIAgEhMSBAAAAQEYAiIJCcgByAEK6AcAEhMSBAAAAQEYAiIJCbAJyAEKAKAGEhQSBAAAAQEYAiIKCbAJ6AcK5wefBhIWEgQAAAEBGAIiDAmAGYgOEpADANgEABoFY2xhc3MaBm9uZXdheSIHCgVtaW5vciIFCgN5ZXM=";
        let no_return = "GkkKDnRyYW5zcG9ydGF0aW9ueAIogCASExIEAAABARgCIgkJyAHIAQroBwAaBWNsYXNzGgZvbmV3YXkiBwoFbWlub3IiBQoDeWVz";
        let tile = fixture()["tileBindings"][0].clone();
        let key = tile["canonical_url"].as_str().unwrap().to_string();
        let setup = |raw: &str| {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw)
                .unwrap();
            let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
            let cached = (
                OfmRawInput {
                    sha256: sha.clone(),
                    byte_length: bytes.len(),
                    base64: raw.into(),
                },
                OfmBindingInput {
                    key: key.clone(),
                    url: key.clone(),
                    raw_sha256: sha.clone(),
                    revision: tile["revision"].as_str().unwrap().into(),
                    z: 14,
                    x: tile["x"].as_u64().unwrap() as u32,
                    y: tile["y"].as_u64().unwrap() as u32,
                    decoder_rule: tile["decoder_rule"].as_str().unwrap().into(),
                    license: "ODbL-1.0".into(),
                    attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                },
            );
            let binding = BusPathBinding {
                rule: BUS_GRAPH_RULE.into(),
                tile_key: key.clone(),
                tile_raw_sha256: sha,
                respect_oneway: true,
                start: BusVertex {
                    feature: 0,
                    line: 0,
                    vertex: 0,
                },
                end: BusVertex {
                    feature: 0,
                    line: 0,
                    vertex: 1,
                },
                additional_tiles: vec![],
                end_tile_key: None,
                pattern: Some(BusPattern::OpenRoundTrip),
            };
            (binding, cached)
        };
        let before = editor.snapshot();
        let history = editor.world_edit_history();
        let token = editor.ofm_world_token().unwrap();
        let (out, cached) = setup(no_return);
        let preview = editor
            .preview_gameplay_ofm_bus(
                &token,
                "B2".into(),
                "往復".into(),
                out.clone(),
                vec![setup(no_return).1],
            )
            .unwrap();
        assert_eq!(preview.points.len(), 2);
        assert!(preview.return_path.is_none());
        assert!(preview
            .return_unavailable_reason
            .unwrap()
            .contains("復路がありません"));
        let error = editor
            .put_gameplay_ofm_bus(&token, "B2".into(), "往復".into(), out, vec![cached])
            .err()
            .unwrap();
        assert!(error.contains("復路がありません"));
        assert_eq!(editor.snapshot(), before);
        assert_eq!(editor.world_edit_history().revision, history.revision);
        assert_eq!(editor.ofm_world_token().unwrap(), token);

        let (binding, cached) = setup(two_way);
        let preview = editor
            .preview_gameplay_ofm_bus(
                &token,
                "B2".into(),
                "往復".into(),
                binding.clone(),
                vec![setup(two_way).1],
            )
            .unwrap();
        assert!(preview.return_unavailable_reason.is_none());
        let returned = preview.return_path.unwrap();
        assert_eq!(returned.points.len(), 3);
        assert_eq!(returned.points.first(), preview.points.last());
        assert_eq!(returned.points.last(), preview.points.first());
        assert!(returned.length_m > preview.length_m);
        assert_eq!(editor.snapshot(), before);
        let accepted = editor
            .put_gameplay_ofm_bus(
                &token,
                "B2".into(),
                "往復".into(),
                binding.clone(),
                vec![cached],
            )
            .unwrap();
        assert_eq!(accepted.road_transport.bus_routes[0].edges.len(), 1);
        assert_eq!(accepted.road_transport.bus_stops.len(), 2);
        let ids: Vec<_> = accepted.road_transport.bus_routes[0]
            .stops
            .iter()
            .map(|s| format!("bus:{}", s.stop_id))
            .collect();
        editor
            .configure_bus_operation(
                "B2".into(),
                Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 0,
                    headway_ms: 60_000,
                },
                Vehicle {
                    max_speed_kmh: 60.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.2,
                    length_m: 12.,
                },
                30.,
                1_000,
                &editor.operation_snapshot().timetable_edit_token,
            )
            .unwrap();
        let runtime = &mut editor.world.operation_mut().unwrap().runtime;
        let spec = runtime.service_spec("bus:B2").unwrap();
        assert_eq!(spec.legs.len(), 2);
        assert_eq!((&spec.legs[0].from, &spec.legs[0].to), (&ids[0], &ids[1]));
        assert_eq!((&spec.legs[1].from, &spec.legs[1].to), (&ids[1], &ids[0]));
        assert_eq!(spec.legs[0].sections[0].resources.len(), 1);
        assert_eq!(spec.legs[1].sections[0].resources.len(), 2);
        assert!(spec.legs[1].sections[0]
            .resources
            .iter()
            .all(|key| key.starts_with("road:edge:ofm-bus:edge:")));
        assert!(spec.legs[1].sections[0].length_m > spec.legs[0].sections[0].length_m);
        assert!((spec.legs[1].sections[0].length_m - returned.length_m).abs() < 0.1);
        runtime.advance_to(1000, 100).unwrap();
        let active = editor.world.operation_save().unwrap();
        let selected = |feature, vertex| BusVertex {
            feature,
            line: 0,
            vertex,
        };

        // A separate live edit can grow an unrelated route without changing
        // this active round trip's compiled specs, train key, or clock.
        let mut other_proof = binding.clone();
        other_proof.pattern = None;
        other_proof.start = selected(3, 0);
        other_proof.end = selected(3, 1);
        let token = editor.ofm_world_token().unwrap();
        let one = editor
            .put_gameplay_ofm_bus(
                &token,
                "independent".into(),
                "別区間".into(),
                other_proof.clone(),
                vec![],
            )
            .unwrap();
        other_proof.end = selected(3, 2);
        let token = editor.ofm_world_token().unwrap();
        let with_other = editor
            .put_gameplay_ofm_bus(
                &token,
                "independent".into(),
                "別区間".into(),
                other_proof,
                vec![],
            )
            .unwrap();
        assert_ne!(one.road_transport.edges, with_other.road_transport.edges);
        assert!(!editor.operation_snapshot().invalidated);
        assert_eq!(editor.world.operation_save().unwrap(), active);
        assert_eq!(
            with_other.road_transport.bus_routes[0],
            accepted.road_transport.bus_routes[0]
        );

        // A separately authored outbound path along the return streets must
        // wait, not use duplicate route-local road reservations.
        let mut shadow = editor.clone();
        let mut same_streets = binding.clone();
        same_streets.pattern = None;
        same_streets.start = selected(1, 0);
        same_streets.end = selected(2, 1);
        let shadow_token = shadow.ofm_world_token().unwrap();
        shadow
            .put_gameplay_ofm_bus(
                &shadow_token,
                "compete".into(),
                "同じ復路".into(),
                same_streets,
                vec![],
            )
            .unwrap();
        let primary = active.bus_configs[0].clone();
        let mut contender = primary.clone();
        contender.route_id = "compete".into();
        let [moving] = active.runtime.trains.as_slice() else {
            panic!()
        };
        let rosenfu_core::operation::TrainState::Moving {
            arrival_ms,
            release_ms,
            ..
        } = moving.state
        else {
            panic!()
        };
        let ready = release_ms.max(arrival_ms + primary.dwell_ms);
        contender.timetable.first_departure_ms = ready;
        contender.timetable.last_departure_ms = ready;
        assert_eq!(
            shadow.world.compile_bus_operation(&contender).unwrap().legs[0].sections[0].resources,
            active.runtime.services[0].legs[1].sections[0].resources,
        );
        shadow
            .world
            .configure_all_operation(vec![], vec![primary, contender])
            .unwrap();
        let runtime = &mut shadow.world.operation_mut().unwrap().runtime;
        runtime.advance_to(ready, 100).unwrap();
        use rosenfu_core::operation::TrainState;
        assert!(runtime
            .trains()
            .any(|train| train.key.service_id == "bus:B2"
                && train.leg == 1
                && matches!(train.state, TrainState::Moving { .. })));
        assert!(runtime
            .trains()
            .any(|train| train.key.service_id == "bus:compete"
                && matches!(train.state, TrainState::Waiting { .. })));
        let finished = runtime.advance_to(400_000, 100).unwrap();
        assert!(finished
            .arrivals
            .iter()
            .any(|arrival| arrival.train.service_id == "bus:compete" && arrival.delay_ms > 0));

        for (from, to) in [(&ids[0], &ids[1]), (&ids[1], &ids[0])] {
            let od = editor
                .preview_fixed_od(from.clone(), to.clone(), Some(50.), None)
                .unwrap();
            assert!(od.search_complete);
            assert!((od.rail.iter().map(|path| path.trips).sum::<f64>() - 50.).abs() < 1e-9);
        }
        let saved = editor.world.operation_save().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("ordinary-open-ofm-bus.rosenfu");
        editor.save(&file).unwrap();
        editor.create_new_world("Other".into()).unwrap();
        editor.load(&file).unwrap();
        assert_eq!(editor.snapshot(), with_other);
        assert_eq!(editor.world.operation_save().unwrap(), saved);
        let finished = editor
            .world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(400_000, 100)
            .unwrap();
        assert_eq!(
            finished
                .arrivals
                .iter()
                .map(|x| &x.station_id)
                .collect::<Vec<_>>(),
            [&ids[1], &ids[0]]
        );
        assert_eq!(finished.arrivals[0].train, finished.arrivals[1].train);
    }

    #[test]
    fn ordinary_ofm_bus_commits_routes_operates_and_reopens_without_pilot() {
        use rosenfu_core::ofm_evidence::{BusPathBinding, BusVertex, BUS_GRAPH_RULE};
        use rosenfu_core::operation::{Timetable, Vehicle};
        let mut editor = WorldEditor::new();
        editor.rename_company("普通のバス会社".into()).unwrap();
        // Generated MVT: two class=minor ground roads, with a shared endpoint
        // on the first road's interior vertex. No regional pilot or QA fixture.
        let raw = "GlEKDnRyYW5zcG9ydGF0aW9ueAIogCASFBICAAAYAiIMCcgByAESyAEAyAEAEhQSAgAAGAIiDAmQA8gBEgDIAQDIARoFY2xhc3MiBwoFbWlub3I=";
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .unwrap();
        let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
        let tile = fixture()["tileBindings"][0].clone();
        let key = tile["canonical_url"].as_str().unwrap().to_string();
        let cached = || {
            (
                OfmRawInput {
                    sha256: sha.clone(),
                    byte_length: bytes.len(),
                    base64: raw.into(),
                },
                OfmBindingInput {
                    key: key.clone(),
                    url: key.clone(),
                    raw_sha256: sha.clone(),
                    revision: tile["revision"].as_str().unwrap().into(),
                    z: 14,
                    x: tile["x"].as_u64().unwrap() as u32,
                    y: tile["y"].as_u64().unwrap() as u32,
                    decoder_rule: tile["decoder_rule"].as_str().unwrap().into(),
                    license: "ODbL-1.0".into(),
                    attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                },
            )
        };
        let from = BusVertex {
            feature: 0,
            line: 0,
            vertex: 0,
        };
        let to = BusVertex {
            feature: 1,
            line: 0,
            vertex: 2,
        };
        let request = BusPathBinding {
            rule: BUS_GRAPH_RULE.into(),
            tile_key: key.clone(),
            tile_raw_sha256: sha.clone(),
            respect_oneway: false,
            start: from,
            end: to,
            additional_tiles: vec![],
            end_tile_key: None,
            pattern: None,
        };
        let before = editor.snapshot();
        let mut forged = request.clone();
        forged.tile_raw_sha256 = "0".repeat(64);
        let token = editor.ofm_world_token().unwrap();
        for (expected, proposed) in [("stale", request.clone()), (&token, forged)] {
            assert!(editor
                .put_gameplay_ofm_bus(
                    expected,
                    "B1".into(),
                    "普通のバス".into(),
                    proposed,
                    vec![cached()]
                )
                .is_err());
            assert_eq!(editor.snapshot(), before);
        }
        let mut no_path = request.clone();
        no_path.end.vertex = 500;
        assert!(editor
            .put_gameplay_ofm_bus(
                &token,
                "B1".into(),
                "普通のバス".into(),
                no_path,
                vec![cached()],
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        assert!(before.road_evidence_descriptors.is_empty());
        let preview = editor
            .preview_gameplay_ofm_bus(
                &token,
                "B1".into(),
                "普通のバス".into(),
                request.clone(),
                vec![cached()],
            )
            .unwrap();
        assert_eq!(editor.snapshot(), before);
        assert_eq!(editor.ofm_world_token().unwrap(), token);
        let accepted = editor
            .put_gameplay_ofm_bus(
                &token,
                "B1".into(),
                "普通のバス".into(),
                request,
                vec![cached()],
            )
            .unwrap();
        assert!(accepted.construction_plans.is_none());
        assert_eq!(accepted.road_evidence_descriptors.len(), 1);
        assert_eq!(accepted.road_transport.bus_routes[0].stops.len(), 2);
        let leg = accepted.road_transport.bus_legs(&accepted, "B1").unwrap();
        assert_eq!(leg.len(), 1);
        assert_eq!(leg[0].traversed_edges.len(), 3);
        assert!(leg[0].length_m > 50.);
        assert_eq!(preview.length_m, leg[0].length_m);
        assert_eq!(preview.points.len(), 4);
        let first_edge = &accepted.road_transport.bus_routes[0].edges[0];
        let first_edge = accepted
            .road_transport
            .edges
            .iter()
            .find(|e| &e.id == first_edge)
            .unwrap();
        assert_eq!(preview.points[0], first_edge.geometry[0]);
        assert_eq!(editor.undo_world_edit(accepted.clone()).unwrap(), before);
        assert_eq!(editor.redo_world_edit(before.clone()).unwrap(), accepted);
        editor
            .configure_bus_operation(
                "B1".into(),
                Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 60000,
                    headway_ms: 60000,
                },
                Vehicle {
                    max_speed_kmh: 60.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.2,
                    length_m: 12.,
                },
                30.,
                10000,
                &editor.operation_snapshot().timetable_edit_token,
            )
            .unwrap();
        editor.set_operation_clock(false, 1).unwrap();
        let runtime = &mut editor.world.operation_mut().unwrap().runtime;
        let moving = runtime.advance_to(1000, 1000).unwrap();
        assert!(moving.arrivals.is_empty());
        assert_eq!(editor.operation_snapshot().trains.len(), 1);
        let folder = tempfile::tempdir().unwrap();
        // B2 overlaps both of B1's vertical atoms. Adding it must not
        // invalidate B1's already moving run or re-key its stops/resources.
        let primary = editor.operation_snapshot().trains[0].clone();
        let mut second = accepted.ofm_gameplay_bus_routes["B1"].clone();
        second.start = BusVertex {
            feature: 1,
            line: 0,
            vertex: 0,
        };
        let token = editor.ofm_world_token().unwrap();
        let second_preview = editor
            .preview_gameplay_ofm_bus(
                &token,
                "B2".into(),
                "重なる普通バス".into(),
                second.clone(),
                vec![],
            )
            .unwrap();
        assert_eq!(second_preview.points, preview.points[1..]);
        let two = editor
            .put_gameplay_ofm_bus(&token, "B2".into(), "重なる普通バス".into(), second, vec![])
            .unwrap();
        assert_eq!(two.ofm_gameplay_bus_routes.len(), 2);
        assert_eq!(two.road_transport.edges.len(), 3);
        let [major, minor] = two.road_transport.bus_routes.as_slice() else {
            panic!()
        };
        assert_eq!(major, &accepted.road_transport.bus_routes[0]);
        assert_eq!(minor.edges, major.edges[1..]);
        assert_ne!(minor.stops[1].stop_id, major.stops[1].stop_id);
        assert!(!editor.operation_snapshot().invalidated);
        assert_eq!(editor.operation_snapshot().trains, [primary]);

        // Simulate a valid save from a slightly different inverse Mercator
        // implementation. Adding another route keeps already stored coordinates.
        let mut drifted = accepted.clone();
        for node in &mut drifted.road_transport.nodes {
            node.point.latitude += 1e-11;
        }
        for edge in &mut drifted.road_transport.edges {
            for point in &mut edge.geometry {
                point.latitude += 1e-11;
            }
        }
        let drifted_world = World::new(drifted.clone()).unwrap();
        let extended = drifted_world
            .candidate_with_gameplay_ofm_bus(
                editor.company,
                "B2".into(),
                "別の系統".into(),
                two.ofm_gameplay_bus_routes["B2"].clone(),
                vec![],
            )
            .unwrap();
        assert_eq!(
            extended.snapshot().road_transport.edges,
            drifted.road_transport.edges
        );

        let multi_path = folder.path().join("shared-two-buses.rosenfu");
        editor.clone().save(&multi_path).unwrap();
        let mut two_reopened = WorldEditor::new();
        two_reopened.load(&multi_path).unwrap();
        assert_eq!(two_reopened.snapshot(), two);
        assert_eq!(two_reopened.operation_snapshot().trains.len(), 1);

        // A separately initialized timetable instantiates both simultaneous
        // departures. It must reserve the actual same edge, not route-local IDs.
        use rosenfu_core::operation::TrainState;
        let mut two_running = two_reopened.world.clone();
        let one = two_running.operation().unwrap().bus_configs[0].clone();
        let mut other = one.clone();
        other.route_id = "B2".into();
        two_running
            .configure_all_operation(vec![], vec![one, other])
            .unwrap();
        let runtime = &mut two_running.operation_mut().unwrap().runtime;
        runtime.set_paused(false);
        runtime.advance_to(1000, 1000).unwrap();
        assert_eq!(
            runtime
                .trains()
                .filter(|t| matches!(t.state, TrainState::Moving { .. }))
                .count(),
            1
        );
        assert_eq!(
            runtime
                .trains()
                .filter(|t| matches!(t.state, TrainState::Waiting { .. }))
                .count(),
            1
        );
        assert_eq!(runtime.advance_to(59999, 1000).unwrap().arrivals.len(), 2);

        let mut forged: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&multi_path).unwrap()).unwrap();
        forged["world"]["road_transport"]["bus_routes"][1]["edges"][0] =
            major.edges[0].clone().into();
        let bytes = serde_json::to_vec(&forged).unwrap();
        std::fs::write(&multi_path, &bytes).unwrap();
        assert!(two_reopened.load(&multi_path).is_err());
        assert_eq!(two_reopened.snapshot(), two);
        assert_eq!(std::fs::read(&multi_path).unwrap(), bytes);

        // Removing B1 prunes only its unique approach; B2's stops and shared
        // track segments survive. Undo reconstitutes the exact graph/evidence.
        let remainder = two_reopened
            .remove_gameplay_ofm_bus(&two_reopened.ofm_world_token().unwrap(), "B1")
            .unwrap();
        assert_eq!(remainder.road_transport.edges.len(), 2);
        assert_eq!(
            &remainder.road_transport.bus_routes,
            [minor.clone()].as_slice()
        );
        assert_eq!(
            remainder.road_evidence_descriptors,
            two.road_evidence_descriptors
        );
        assert_eq!(two_reopened.undo_world_edit(remainder).unwrap(), two);
        let clean_for_first = editor
            .remove_gameplay_ofm_bus(&editor.ofm_world_token().unwrap(), "B2")
            .unwrap();
        assert_eq!(clean_for_first, accepted);
        assert!(!editor.operation_snapshot().invalidated);
        let file = folder.path().join("ordinary-ofm-bus.rosenfu");
        editor.save(&file).unwrap();
        let good = std::fs::read(&file).unwrap();
        let mut reopened = WorldEditor::new();
        reopened.load(&file).unwrap();
        assert_eq!(reopened.snapshot(), accepted);
        assert_eq!(reopened.operation_snapshot().trains.len(), 1);
        let runtime = &mut reopened.world.operation_mut().unwrap().runtime;
        runtime.set_paused(false);
        let finished = runtime.advance_to(59999, 1000).unwrap();
        assert_eq!(finished.arrivals.len(), 1);
        let mut tampered: serde_json::Value = serde_json::from_slice(&good).unwrap();
        tampered["world"]["ofm_gameplay_bus_routes"]["B1"]["tileRawSha256"] = "0".repeat(64).into();
        let bad = serde_json::to_vec(&tampered).unwrap();
        std::fs::write(&file, &bad).unwrap();
        assert!(reopened.load(&file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), bad);
        assert_eq!(reopened.snapshot(), accepted);
        let cleared = reopened
            .remove_gameplay_ofm_bus(&reopened.ofm_world_token().unwrap(), "B1")
            .unwrap();
        assert!(cleared.ofm_gameplay_bus_routes.is_empty());
        assert!(cleared.road_transport.edges.is_empty());
        assert_eq!(
            cleared.road_evidence_descriptors,
            accepted.road_evidence_descriptors
        );
    }

    #[test]
    fn ordinary_ofm_bus_rectangle_atomic_cache_roundtrip_preserves_all_three_tiles() {
        use rosenfu_core::ofm_evidence::{
            BusPathBinding, BusTileProof, BusVertex, BUS_RECTANGLE_RULE,
        };
        let mut editor = WorldEditor::new();
        editor.rename_company("広域バス会社".into()).unwrap();
        let fixture = fixture();
        let template = &fixture["tileBindings"][0];
        let rev = template["revision"].as_str().unwrap();
        let decoder = template["decoder_rule"].as_str().unwrap();
        let y = template["y"].as_u64().unwrap() as u32;
        // Generated raw MVT fixture: the middle road has only two buffered
        // points, yet both independently proven border joins are traversable.
        let input = [
            "GjgKDnRyYW5zcG9ydGF0aW9ueAIogCASERICAAAYAiIJCYAytB8K0A8AGgVjbGFzcyIHCgVtaW5vcg==",
            "GjgKDnRyYW5zcG9ydGF0aW9ueAIogCASERICAAAYAiIJCc8BtB8KoEMAGgVjbGFzcyIHCgVtaW5vcg==",
            "GjgKDnRyYW5zcG9ydGF0aW9ueAIogCASERICAAAYAiIJCc8BtB8K0A8AGgVjbGFzcyIHCgVtaW5vcg==",
        ];
        let cached = || {
            input
                .iter()
                .enumerate()
                .map(|(i, &raw)| {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(raw)
                        .unwrap();
                    let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
                    let x = 14552 + i as u32;
                    let key = format!("https://tiles.openfreemap.org/planet/{rev}/14/{x}/{y}.pbf");
                    (
                        OfmRawInput {
                            sha256: sha.clone(),
                            byte_length: bytes.len(),
                            base64: raw.into(),
                        },
                        OfmBindingInput {
                            key: key.clone(),
                            url: key,
                            raw_sha256: sha,
                            revision: rev.into(),
                            z: 14,
                            x,
                            y,
                            decoder_rule: decoder.into(),
                            license: "ODbL-1.0".into(),
                            attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let all = cached();
        let request = BusPathBinding {
            rule: BUS_RECTANGLE_RULE.into(),
            tile_key: all[0].1.key.clone(),
            tile_raw_sha256: all[0].0.sha256.clone(),
            end_tile_key: Some(all[2].1.key.clone()),
            pattern: None,
            additional_tiles: all[1..]
                .iter()
                .map(|(raw, b)| BusTileProof {
                    tile_key: b.key.clone(),
                    tile_raw_sha256: raw.sha256.clone(),
                })
                .collect(),
            respect_oneway: false,
            start: BusVertex {
                feature: 0,
                line: 0,
                vertex: 0,
            },
            end: BusVertex {
                feature: 0,
                line: 0,
                vertex: 1,
            },
        };
        let before = editor.snapshot();
        let token = editor.ofm_world_token().unwrap();
        let mut incomplete = cached();
        incomplete.remove(1);
        assert!(editor
            .put_gameplay_ofm_bus(
                &token,
                "R3".into(),
                "3セル系統".into(),
                request.clone(),
                incomplete
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let preview = editor
            .preview_gameplay_ofm_bus(
                &token,
                "R3".into(),
                "3セル系統".into(),
                request.clone(),
                cached(),
            )
            .unwrap();
        assert_eq!(preview.points.len(), 4);
        assert_eq!(editor.snapshot(), before);
        let created = editor
            .put_gameplay_ofm_bus(&token, "R3".into(), "3セル系統".into(), request, cached())
            .unwrap();
        assert_eq!(created.road_evidence_descriptors.len(), 3);
        assert_eq!(created.road_transport.edges.len(), 3);
        for (id, (raw, _)) in created.road_transport.bus_routes[0].edges.iter().zip(all) {
            assert_eq!(
                created
                    .road_transport
                    .edges
                    .iter()
                    .find(|e| &e.id == id)
                    .unwrap()
                    .revision,
                raw.sha256
            );
        }
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("rectangle.rosenfu");
        editor.save(&p).unwrap();
        let mut reopen = WorldEditor::new();
        reopen.load(&p).unwrap();
        assert_eq!(reopen.snapshot(), created);
        let replay = reopen
            .preview_gameplay_ofm_bus(
                &reopen.ofm_world_token().unwrap(),
                "R3".into(),
                "3セル系統".into(),
                created.ofm_gameplay_bus_routes["R3"].clone(),
                vec![],
            )
            .unwrap();
        assert_eq!(replay, preview);
    }

    fn request() -> PlannedAlignment {
        serde_json::from_value(serde_json::json!({
            "name":"境界道路", "points":[{"latitude":0.,"longitude":0.},{"latitude":0.,"longitude":1.}],
            "trackType":"normal","stations":[],"ofmPath":fixture()["path"]
        })).unwrap()
    }
    #[test]
    fn parked_evidence_build_and_deletion_undo_roundtrip() {
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let parked = editor.snapshot();
        assert_eq!(parked.road_evidence_descriptors.len(), 2);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
        editor.build_plan(vec![request()]).unwrap();
        let built = editor.snapshot();
        let line = built.alignments.last().unwrap();
        assert!(line.points.iter().all(|p| p.latitude > 30.)); // client coordinates ignored
        assert_eq!(built.road_path_provenance.len(), 1);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
        editor.delete_alignment(line.id).unwrap();
        assert_eq!(editor.undo_alignment_deletion().unwrap(), built);
    }
    #[test]
    fn seam_exact_endpoint_extension_keeps_existing_alignment_and_provenance() {
        use rosenfu_core::world::AlignmentEndpoint;
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let points: Vec<GeoPoint> =
            serde_json::from_value(fixture()["plans"]["parkedPaths"][0]["points"].clone()).unwrap();
        let start = points[0];
        let source = editor
            .build_alignment_typed_unchecked(
                "元線路".into(),
                vec![
                    GeoPoint {
                        longitude: start.longitude - 0.002,
                        latitude: start.latitude,
                    },
                    start,
                ],
                rosenfu_core::world::TrackType::Normal,
            )
            .unwrap();
        let mut r = request();
        r.extension = Some(PlannedAlignmentExtension {
            alignment_id: source,
            endpoint: AlignmentEndpoint::End,
        });
        editor.build_plan(vec![r]).unwrap();
        let built = editor.snapshot();
        assert_eq!(built.alignments.len(), 1);
        assert_eq!(built.alignments[0].id, source);
        assert_eq!(built.alignments[0].points[1..], points);
        assert_eq!(built.road_path_provenance.len(), 1);
        assert_eq!(built.road_path_provenance[0].alignment_id, source);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
    }

    #[test]
    fn seam_destination_only_connects_without_changing_existing_line() {
        use rosenfu_core::world::{AlignmentEndpoint, TrackType};
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let points: Vec<GeoPoint> =
            serde_json::from_value(fixture()["plans"]["parkedPaths"][0]["points"].clone()).unwrap();
        let end = *points.last().unwrap();
        let target = editor
            .build_alignment_typed_unchecked(
                "接続先".into(),
                vec![
                    end,
                    GeoPoint {
                        longitude: end.longitude + 0.002,
                        latitude: end.latitude,
                    },
                ],
                TrackType::Normal,
            )
            .unwrap();
        let original = editor.snapshot().alignments[0].clone();
        let mut r = request();
        r.destination = Some(PlannedAlignmentExtension {
            alignment_id: target,
            endpoint: AlignmentEndpoint::Start,
        });
        editor.build_plan(vec![r]).unwrap();
        let built = editor.snapshot();
        assert_eq!(built.alignments.len(), 2);
        assert_eq!(built.alignments[0], original);
        assert_eq!(built.alignment_connections.len(), 1);
        assert_eq!(built.road_path_provenance.len(), 1);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
    }

    #[test]
    fn invalid_seam_destination_rolls_back_world_plans_and_evidence() {
        use rosenfu_core::world::{AlignmentEndpoint, TrackType};
        for gap in [0.001, -0.002] {
            let mut editor = WorldEditor::new();
            stage(&mut editor);
            let points: Vec<GeoPoint> =
                serde_json::from_value(fixture()["plans"]["parkedPaths"][0]["points"].clone())
                    .unwrap();
            let mut end = *points.last().unwrap();
            // A separated endpoint or a cusp cannot become a connection.
            if gap > 0. {
                end.longitude += gap;
            }
            let target = editor
                .build_alignment_typed_unchecked(
                    "接続先".into(),
                    vec![
                        end,
                        GeoPoint {
                            longitude: end.longitude + gap,
                            latitude: end.latitude,
                        },
                    ],
                    TrackType::Normal,
                )
                .unwrap();
            let before = editor.snapshot();
            let mut r = request();
            r.destination = Some(PlannedAlignmentExtension {
                alignment_id: target,
                endpoint: AlignmentEndpoint::Start,
            });
            assert!(editor.build_plan(vec![r]).is_err());
            assert_eq!(editor.snapshot(), before);
        }
    }

    #[test]
    fn seam_anchor_profile_connects_both_ends_and_preserves_sources() {
        use rosenfu_core::world::{AlignmentEndpoint, InfrastructureLevel, TrackType};
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let points: Vec<GeoPoint> =
            serde_json::from_value(fixture()["plans"]["parkedPaths"][0]["points"].clone()).unwrap();
        let start = points[0];
        let end = *points.last().unwrap();
        let source = editor
            .build_alignment_typed_unchecked(
                "元線路".into(),
                vec![
                    GeoPoint {
                        longitude: start.longitude - 0.002,
                        latitude: start.latitude,
                    },
                    start,
                ],
                TrackType::Normal,
            )
            .unwrap();
        let target = editor
            .build_alignment_typed_unchecked(
                "接続先".into(),
                vec![
                    end,
                    GeoPoint {
                        longitude: end.longitude + 0.002,
                        latitude: end.latitude,
                    },
                ],
                TrackType::Normal,
            )
            .unwrap();
        let originals = editor.snapshot().alignments;
        let mut r = request();
        r.extension = Some(PlannedAlignmentExtension {
            alignment_id: source,
            endpoint: AlignmentEndpoint::End,
        });
        r.destination = Some(PlannedAlignmentExtension {
            alignment_id: target,
            endpoint: AlignmentEndpoint::Start,
        });
        r.anchor_levels = Some(vec![InfrastructureLevel::Ground; 2]);
        editor.build_plan(vec![r]).unwrap();
        let built = editor.snapshot();
        assert_eq!(built.alignments[..2], originals);
        assert_eq!(built.alignments.len(), 3);
        assert_eq!(built.alignment_connections.len(), 2);
        assert_eq!(built.road_path_provenance.len(), 1);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
    }

    #[test]
    fn seam_anchor_heights_preserve_provenance_save_and_undo() {
        use rosenfu_core::world::InfrastructureLevel;
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let mut r = request();
        r.anchor_levels = Some(vec![InfrastructureLevel::Elevated; 2]);
        r.infrastructure.level = InfrastructureLevel::Elevated;
        editor.build_plan(vec![r]).unwrap();
        let built = editor.snapshot();
        let line = built.alignments.last().unwrap();
        assert_eq!(line.section_profiles.len(), 1);
        assert_eq!(line.section_profiles[0].start_height_m, 6.);
        assert_eq!(line.section_profiles[0].end_height_m, 6.);
        assert_eq!(built.road_path_provenance.len(), 1);
        assert_eq!(
            rosenfu_storage::decode(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap(),
            editor.world
        );
        editor.delete_alignment(line.id).unwrap();
        assert_eq!(editor.undo_alignment_deletion().unwrap(), built);
    }

    #[test]
    fn seam_steep_or_mismatched_anchor_heights_are_atomic() {
        use rosenfu_core::world::InfrastructureLevel;
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let before = editor.snapshot();
        let mut r = request();
        r.anchor_levels = Some(vec![
            InfrastructureLevel::Ground,
            InfrastructureLevel::Elevated3,
        ]);
        assert!(editor.build_plan(vec![r]).unwrap_err().contains("勾配"));
        assert_eq!(editor.snapshot(), before);
        let mut r = request();
        r.anchor_levels = Some(vec![InfrastructureLevel::Ground]);
        assert!(editor
            .build_plan(vec![r])
            .unwrap_err()
            .contains("高さ指定数"));
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn unsupported_old_save_leaves_file_and_session_unchanged() {
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let before = editor.snapshot();
        let mut old: serde_json::Value =
            serde_json::from_slice(&rosenfu_storage::encode(&editor.world).unwrap()).unwrap();
        old["schema_version"] = serde_json::json!(24);
        let bytes = serde_json::to_vec(&old).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("old.json");
        std::fs::write(&path, &bytes).unwrap();
        assert!(editor.load(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(editor.snapshot(), before);
    }
    /// Optional observed-data acceptance; supply the local pinned corpus explicitly.
    #[test]
    #[ignore = "requires pinned external Tokyo tile corpus, never downloaded by the test"]
    fn observed_tiles_stage_build_save_reload_and_undo() {
        let path =
            std::env::var("OFM_REAL_SESSION_FIXTURE").expect("explicit corpus path required");
        let f: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let start = std::time::Instant::now();
        let mut editor = WorldEditor::new();
        stage_fixture(&mut editor, f.clone());
        let mut r = request();
        r.ofm_path = Some(serde_json::from_value(f["path"].clone()).unwrap());
        editor.build_plan(vec![r]).unwrap();
        let built = editor.snapshot();
        let tmp = tempfile::tempdir().unwrap();
        let save = tmp.path().join("real.json");
        editor.save(&save).unwrap();
        let bytes = std::fs::read(&save).unwrap();
        let mut restored = WorldEditor::new();
        restored.load(&save).unwrap();
        assert_eq!(restored.snapshot(), built);
        let id = built.alignments.last().unwrap().id;
        restored.delete_alignment(id).unwrap();
        assert_eq!(restored.undo_alignment_deletion().unwrap(), built);
        eprintln!(
            "observed session: {} bytes, {:?}",
            bytes.len(),
            start.elapsed()
        );
    }

    #[test]
    fn stale_stage_and_invalid_second_path_preserve_world() {
        let mut editor = WorldEditor::new();
        stage(&mut editor);
        let before = editor.snapshot();
        assert!(editor
            .stage_ofm_construction_plan(
                "stale",
                serde_json::from_value(fixture()["plans"].clone()).unwrap(),
                vec![],
                vec![]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
        let mut bad = request();
        bad.ofm_path.as_mut().unwrap().start.fraction = 2.;
        assert!(editor.build_plan(vec![request(), bad]).is_err());
        assert_eq!(editor.snapshot(), before);
        let raw = OfmRawInput {
            sha256: "0".repeat(64),
            byte_length: 1,
            base64: "AB==".into(),
        };
        assert!(editor
            .stage_ofm_construction_plan(
                &editor.ofm_world_token().unwrap(),
                serde_json::from_value(fixture()["plans"].clone()).unwrap(),
                vec![raw],
                vec![]
            )
            .is_err());
        assert_eq!(editor.snapshot(), before);
    }

    #[test]
    fn evidence_token_is_read_only_and_changes_with_world_or_slot() {
        let mut editor = WorldEditor::new();
        let before = editor.snapshot();
        let token = editor.ofm_world_token().unwrap();
        assert_eq!(token, editor.ofm_world_token().unwrap());
        assert_eq!(editor.snapshot(), before);
        editor.active_slot_id = Some("another-slot".into());
        assert_ne!(token, editor.ofm_world_token().unwrap());
        editor.active_slot_id = None;
        assert_eq!(token, editor.ofm_world_token().unwrap());
        editor.add_station("新駅".into(), 35.0, 139.0).unwrap();
        assert_ne!(token, editor.ofm_world_token().unwrap());
    }

    #[test]
    fn ui_preview_restores_current_evidence_but_rejects_changed_manifest() {
        use rosenfu_core::ofm_evidence::{
            raw_sha256, EvidenceStore, RawEvidence, TileBinding, DECODER_RULE,
        };
        let mut layer = vec![10, 14];
        layer.extend(b"transportation");
        layer.extend([120, 2, 40, 128, 32]);
        let mut raw = vec![26, layer.len() as u8];
        raw.extend(layer);
        let sha = raw_sha256(&raw);
        let store = EvidenceStore::from_parts(
            vec![RawEvidence {
                sha256: sha.clone(),
                bytes: raw,
            }],
            vec![TileBinding {
                canonical_url: "https://tiles.openfreemap.org/planet/test/14/1/2.pbf".into(),
                revision: "test".into(),
                z: 14,
                x: 1,
                y: 2,
                raw_sha256: sha,
                decoder_rule: DECODER_RULE.into(),
            }],
        )
        .unwrap();
        let mut editor = WorldEditor::new();
        let old_token = editor.ofm_world_token().unwrap();
        editor.world = World::new(editor.snapshot().with_road_evidence(store).unwrap()).unwrap();
        assert_ne!(old_token, editor.ofm_world_token().unwrap());
        let original = editor.snapshot();
        let mut from_ui: WorldDraft =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert!(World::new(from_ui.clone()).is_err());
        assert_eq!(
            editor
                .world_for_ui_preview(from_ui.clone())
                .unwrap()
                .snapshot(),
            &original
        );
        from_ui.road_evidence_descriptors.clear();
        assert!(editor.world_for_ui_preview(from_ui).is_err());
        assert_eq!(editor.snapshot(), original);
    }

    #[test]
    fn ui_preview_does_not_replace_the_current_world() {
        let editor = WorldEditor::new();
        let original = editor.snapshot();
        let preview = editor.world_for_ui_preview(original.clone()).unwrap();
        assert_eq!(preview.snapshot(), &original);
        assert_eq!(editor.snapshot(), original);
    }
}
