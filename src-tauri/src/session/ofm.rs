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
        self.commit_edit(candidate, false)?;
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
    fn ordinary_closed_bus_uses_one_origin_and_a_real_intermediate_stop_for_one_lap() {
        use rosenfu_core::ofm_evidence::{
            BusInteriorStop, BusPathBinding, BusPattern, BusVertex, BusVia, BUS_GRAPH_RULE,
        };
        use rosenfu_core::operation::{Timetable, Vehicle};

        let mut editor = WorldEditor::new();
        editor.rename_company("環状バス".into()).unwrap();
        let raw = "GowBCg50cmFuc3BvcnRhdGlvbngCKIAgEhMSBAAAAQEYAiIJCcgByAEK6AcAEhMSBAAAAQEYAiIJCbAJyAEKAKAGEhQSBAAAAQEYAiIKCbAJ6AcK5wefBhIWEgQAAAEBGAIiDAmAGYgOEpADANgEABoFY2xhc3MaBm9uZXdheSIHCgVtaW5vciIFCgN5ZXM=";
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .unwrap();
        let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
        let tile = fixture()["tileBindings"][0].clone();
        let key = tile["canonical_url"].as_str().unwrap().to_string();
        let cached = || {
            vec![(
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
            )]
        };
        let at = |feature, vertex| BusVertex {
            feature,
            line: 0,
            vertex,
        };
        let request = BusPathBinding {
            rule: BUS_GRAPH_RULE.into(),
            grade_policy: None,
            tile_key: key.clone(),
            tile_raw_sha256: sha.clone(),
            respect_oneway: true,
            start: at(0, 0),
            end: at(0, 0),
            additional_tiles: vec![],
            end_tile_key: None,
            pattern: Some(BusPattern::ClosedOneWay),
            via: vec![BusVia {
                id: "first".into(),
                tile_key: key.clone(),
                vertex: at(0, 1),
            }],
            interior_stops: vec![],
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap()["pattern"],
            "closedOneWay"
        );
        let token = editor.ofm_world_token().unwrap();
        let before = editor.snapshot();
        let history = editor.world_edit_history();
        let mut no_via = request.clone();
        no_via.via.clear();
        let mut wrong_end = request.clone();
        wrong_end.end = at(1, 1);
        let mut zero = request.clone();
        zero.via[0].vertex = request.start;
        for (candidate, text) in [(no_via, "環状線"), (wrong_end, "環状線"), (zero, "第1区間")]
        {
            let error = editor
                .put_gameplay_ofm_bus(
                    &token,
                    "circle".into(),
                    "町内一周".into(),
                    candidate,
                    cached(),
                )
                .err()
                .unwrap();
            assert!(error.contains(text), "{error}");
            assert_eq!(editor.snapshot(), before);
            assert_eq!(editor.world_edit_history().revision, history.revision);
        }

        let preview = editor
            .preview_gameplay_ofm_bus_details(
                &token,
                "circle".into(),
                "町内一周".into(),
                request.clone(),
                cached(),
                crate::session::GameplayBusPreviewOptions {
                    request_stop_targets: true,
                    request_auto_stop_suggestions: true,
                },
            )
            .unwrap();
        assert_eq!(preview.points.len(), 4);
        assert_eq!(preview.points.first(), preview.points.last());
        assert!(preview.length_m > 0.);
        assert!(preview.return_path.is_none() && preview.return_unavailable_reason.is_none());
        assert_eq!(preview.via_point_indices, [1]);
        let wire = serde_json::to_value(&preview).unwrap();
        assert!(wire.get("loopFallbackStop").is_some());
        assert!(wire.get("returnPath").is_none());
        let fallback = preview.loop_fallback_stop.unwrap();
        assert_ne!(Some(&fallback.point), preview.points.first());
        assert_eq!(editor.snapshot(), before);

        let mut manual = request.clone();
        manual.interior_stops.push(BusInteriorStop {
            id: "manual".into(),
            name: "通りの停留所".into(),
            occurrence_key: preview.stop_occurrence_keys[0].clone(),
            fraction_q: 500_000,
        });
        let manual_show = editor
            .preview_gameplay_ofm_bus(
                &token,
                "circle".into(),
                "町内一周".into(),
                manual.clone(),
                cached(),
            )
            .unwrap();
        assert!(manual_show.loop_fallback_stop.is_none());
        let mut alternate = editor.clone();
        let manual_world = alternate
            .put_gameplay_ofm_bus(&token, "circle".into(), "町内一周".into(), manual, cached())
            .unwrap();
        assert_eq!(manual_world.road_transport.bus_stops.len(), 2);
        assert!(manual_world
            .road_transport
            .bus_stops
            .iter()
            .any(|s| s.name == "通りの停留所"));
        assert!(!manual_world
            .road_transport
            .bus_stops
            .iter()
            .any(|s| s.id == fallback.stop_id));

        let accepted = editor
            .put_gameplay_ofm_bus(
                &token,
                "circle".into(),
                "町内一周".into(),
                request,
                cached(),
            )
            .unwrap();
        let route = &accepted.road_transport.bus_routes[0];
        assert_eq!(route.edges.len(), 3);
        assert_eq!(route.stops.len(), 2);
        assert_eq!(accepted.road_transport.bus_stops.len(), 2);
        assert_eq!(route.stops[1].stop_id, fallback.stop_id);
        let a = &route.stops[0].stop_id;
        let middle = &route.stops[1].stop_id;
        let legs = accepted
            .road_transport
            .bus_legs(&accepted, "circle")
            .unwrap();
        assert_eq!((&legs[0].from_stop_id, &legs[0].to_stop_id), (a, middle));
        assert_eq!((&legs[1].from_stop_id, &legs[1].to_stop_id), (middle, a));
        assert!(legs.iter().all(|leg| leg.length_m > 0.));
        assert!((legs.iter().map(|leg| leg.length_m).sum::<f64>() - preview.length_m).abs() < 1e-9);

        editor
            .configure_bus_operation(
                "circle".into(),
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
                None,
                &editor.operation_snapshot().timetable_edit_token,
            )
            .unwrap();
        editor
            .world
            .operation_mut()
            .unwrap()
            .runtime
            .advance_to(1000, 100)
            .unwrap();
        let active = editor.world.operation_save().unwrap();
        assert_eq!(active.runtime.services[0].legs.len(), 2);
        for (from, to) in [(a, middle), (middle, a)] {
            let od = editor
                .preview_fixed_od(format!("bus:{from}"), format!("bus:{to}"), Some(10.), None)
                .unwrap();
            assert!(od.search_complete);
            assert!((od.rail.iter().map(|p| p.trips).sum::<f64>() - 10.).abs() < 1e-9);
        }
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("closed-bus-loop.rosenfu");
        editor.save(&file).unwrap();
        editor.create_new_world("Other".into()).unwrap();
        editor.load(&file).unwrap();
        assert_eq!(editor.snapshot(), accepted);
        assert_eq!(editor.world.operation_save().unwrap(), active);
        let runtime = &mut editor.world.operation_mut().unwrap().runtime;
        let completed = runtime.advance_to(400_000, 100).unwrap();
        assert_eq!(
            completed
                .arrivals
                .iter()
                .map(|a| a.station_id.as_str())
                .collect::<Vec<_>>(),
            [format!("bus:{middle}"), format!("bus:{a}")]
        );
        assert_eq!(completed.arrivals[0].train, completed.arrivals[1].train);
        assert!(runtime.trains().next().is_none());
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
                grade_policy: None,
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
                interior_stops: vec![],
                via: vec![],
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
                None,
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
            grade_policy: None,
            tile_key: key.clone(),
            tile_raw_sha256: sha.clone(),
            respect_oneway: false,
            start: from,
            end: to,
            additional_tiles: vec![],
            end_tile_key: None,
            pattern: None,
            interior_stops: vec![],
            via: vec![],
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
                None,
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
            BusPathBinding, BusPattern, BusTileProof, BusVertex, BusVia, BUS_RECTANGLE_RULE,
        };
        let mut editor = WorldEditor::new();
        editor.rename_company("広域バス会社".into()).unwrap();
        let fixture = fixture();
        let template = &fixture["tileBindings"][0];
        let rev = template["revision"].as_str().unwrap();
        let decoder = template["decoder_rule"].as_str().unwrap();
        let y = template["y"].as_u64().unwrap() as u32;
        // Generated raw MVT fixture: the middle feature has an authored point
        // at its center and both independently verified border joins.
        let input = [
            "GjgKDnRyYW5zcG9ydGF0aW9ueAIogCASERICAAAYAiIJCYAytB8K0A8AGgVjbGFzcyIHCgVtaW5vcg==",
            "GjsKDnRyYW5zcG9ydGF0aW9ueAIogCASFBICAAAYAiIMCc8BtB8S0CEA0CEAGgVjbGFzcyIHCgVtaW5vcg==",
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
            grade_policy: None,
            tile_key: all[0].1.key.clone(),
            tile_raw_sha256: all[0].0.sha256.clone(),
            end_tile_key: Some(all[2].1.key.clone()),
            pattern: Some(BusPattern::OpenRoundTrip),
            interior_stops: vec![],
            via: vec![BusVia {
                id: "city-centre".into(),
                tile_key: all[1].1.key.clone(),
                vertex: BusVertex {
                    feature: 0,
                    line: 0,
                    vertex: 1,
                },
            }],
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
        let mut blocked_leg = request.clone();
        blocked_leg.end.vertex = 999;
        let failed = editor
            .preview_gameplay_ofm_bus(
                &token,
                "R3".into(),
                "3セル系統".into(),
                blocked_leg.clone(),
                cached(),
            )
            .unwrap();
        assert_eq!(failed.failed_leg_index, Some(1));
        assert!(failed.points.is_empty());
        assert!(editor
            .put_gameplay_ofm_bus(
                &token,
                "R3".into(),
                "3セル系統".into(),
                blocked_leg,
                cached(),
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
        assert_eq!(preview.points.len(), 5);
        assert_eq!(preview.via_point_indices, [2]);
        assert_eq!(
            preview.return_path.as_ref().unwrap().points,
            preview.points.iter().copied().rev().collect::<Vec<_>>()
        );
        assert_eq!(editor.snapshot(), before);
        // Auto is an explicit draft choice, not a save mutation or a
        // replacement of case-specific manually authored stops.
        let suggestions = |binding: BusPathBinding| {
            editor
                .preview_gameplay_ofm_bus_details(
                    &token,
                    "R3".into(),
                    "3セル系統".into(),
                    binding,
                    cached(),
                    GameplayBusPreviewOptions {
                        request_auto_stop_suggestions: true,
                        ..Default::default()
                    },
                )
                .unwrap()
        };
        let auto = suggestions(request.clone());
        assert_eq!(auto.points, preview.points);
        assert_eq!(auto.auto_stop_suggestions.len(), 1);
        assert!(auto.stop_occurrence_keys.is_empty()); // no unnecessary large DTO
        assert_eq!(
            auto.auto_stop_suggestions,
            suggestions(request.clone()).auto_stop_suggestions
        );
        assert_eq!(editor.snapshot(), before);
        let mut with_hand = request.clone();
        let mut hand = auto.auto_stop_suggestions[0].clone();
        hand.id = "keep-existing".into();
        hand.name = "保護する停留所".into();
        with_hand.interior_stops.push(hand);
        let existing = suggestions(with_hand);
        assert_eq!(existing.interior_stops[0].name, "保護する停留所");
        assert!(existing.auto_stop_suggestions.is_empty());
        let mut confirmed = request;
        confirmed.interior_stops = auto.auto_stop_suggestions;
        let confirmed_preview = editor
            .preview_gameplay_ofm_bus(
                &token,
                "R3".into(),
                "3セル系統".into(),
                confirmed.clone(),
                cached(),
            )
            .unwrap();
        assert_eq!(confirmed_preview.points, preview.points);
        assert_eq!(confirmed_preview.interior_stops.len(), 1);
        assert!(confirmed_preview.interior_stops[0].serves_return);
        // Even when the ideal is a control-point boundary, a proposal is
        // strictly on a raw atom interior and never on the turn/node itself.
        assert!(confirmed_preview
            .points
            .iter()
            .all(|point| point != &confirmed_preview.interior_stops[0].point));
        let created = editor
            .put_gameplay_ofm_bus(&token, "R3".into(), "3セル系統".into(), confirmed, cached())
            .unwrap();
        assert_eq!(created.road_transport.bus_routes[0].stops.len(), 3);
        assert_eq!(created.road_evidence_descriptors.len(), 3);
        assert_eq!(created.road_transport.edges.len(), 4);
        let expected = [
            &all[0].0.sha256,
            &all[1].0.sha256,
            &all[1].0.sha256,
            &all[2].0.sha256,
        ];
        for (id, digest) in created.road_transport.bus_routes[0]
            .edges
            .iter()
            .zip(expected)
        {
            assert_eq!(
                &created
                    .road_transport
                    .edges
                    .iter()
                    .find(|e| &e.id == id)
                    .unwrap()
                    .revision,
                digest
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
        assert_eq!(replay, confirmed_preview);
    }

    #[test]
    fn ordinary_bus_strict_return_does_not_force_outbound_control_round_the_loop() {
        use rosenfu_core::ofm_evidence::{
            BusPathBinding, BusPattern, BusVertex, BusVia, BUS_GRAPH_RULE,
        };
        let bytes: [u8; 103] = [
            26, 101, 10, 14, 116, 114, 97, 110, 115, 112, 111, 114, 116, 97, 116, 105, 111, 110,
            120, 2, 40, 128, 32, 18, 22, 18, 4, 0, 0, 1, 1, 24, 2, 34, 12, 9, 200, 1, 200, 1, 18,
            216, 4, 0, 144, 3, 0, 18, 23, 18, 4, 0, 0, 1, 1, 24, 2, 34, 13, 9, 176, 9, 200, 1, 18,
            0, 216, 4, 231, 7, 215, 4, 26, 5, 99, 108, 97, 115, 115, 26, 6, 111, 110, 101, 119, 97,
            121, 34, 7, 10, 5, 109, 105, 110, 111, 114, 34, 5, 10, 3, 121, 101, 115,
        ];
        let raw = base64::engine::general_purpose::STANDARD.encode(bytes);
        let tile = fixture()["tileBindings"][0].clone();
        let key = tile["canonical_url"].as_str().unwrap();
        let digest = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
        let cache = || {
            vec![(
                OfmRawInput {
                    sha256: digest.clone(),
                    byte_length: bytes.len(),
                    base64: raw.clone(),
                },
                OfmBindingInput {
                    key: key.into(),
                    url: key.into(),
                    raw_sha256: digest.clone(),
                    revision: tile["revision"].as_str().unwrap().into(),
                    z: 14,
                    x: tile["x"].as_u64().unwrap() as u32,
                    y: tile["y"].as_u64().unwrap() as u32,
                    decoder_rule: tile["decoder_rule"].as_str().unwrap().into(),
                    license: "ODbL-1.0".into(),
                    attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                },
            )]
        };
        let v = |n| BusVertex {
            feature: 0,
            line: 0,
            vertex: n,
        };
        let proof = BusPathBinding {
            rule: BUS_GRAPH_RULE.into(),
            grade_policy: None,
            tile_key: key.into(),
            tile_raw_sha256: digest.clone(),
            respect_oneway: true,
            start: v(0),
            end: v(2),
            additional_tiles: vec![],
            end_tile_key: None,
            pattern: Some(BusPattern::OpenRoundTrip),
            interior_stops: vec![],
            via: vec![BusVia {
                id: "shape".into(),
                tile_key: key.into(),
                vertex: v(1),
            }],
        };
        let mut editor = WorldEditor::new();
        editor.rename_company("循環バス".into()).unwrap();
        let token = editor.ofm_world_token().unwrap();
        let preview = |p| {
            editor
                .preview_gameplay_ofm_bus(&token, "B1".into(), "折り返し".into(), p, cache())
                .unwrap()
        };
        let strict = preview(proof.clone());
        let home = strict.return_path.unwrap();
        assert_eq!(strict.via_point_indices, [1]);
        assert_eq!(home.points.len(), 3);
        assert_eq!(home.points[0], strict.points[2]);
        assert_eq!(home.points[2], strict.points[0]);
        assert_ne!(home.points[1], strict.points[1]);
        let targets = editor
            .preview_gameplay_ofm_bus_details(
                &token,
                "B1".into(),
                "折り返し".into(),
                proof.clone(),
                cache(),
                GameplayBusPreviewOptions {
                    request_stop_targets: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let mut detour_stop = proof.clone();
        detour_stop
            .interior_stops
            .push(rosenfu_core::ofm_evidence::BusInteriorStop {
                id: "on-out".into(),
                name: "往路の停留所".into(),
                occurrence_key: targets.stop_occurrence_keys[0].clone(),
                fraction_q: 500_000,
            });
        let only_out = preview(detour_stop);
        assert!(only_out.return_path.is_some());
        assert!(!only_out.interior_stops[0].serves_return);
        let mut default = proof;
        default.respect_oneway = false;
        let relaxed = preview(default);
        assert_eq!(
            relaxed.return_path.unwrap().points,
            relaxed.points.into_iter().rev().collect::<Vec<_>>()
        );
    }

    fn verify_ordinary_bus_turnaround(
        source: &WorldEditor,
        mut cfg: rosenfu_core::world::BusOperationConfig,
    ) {
        use rosenfu_core::operation::TrainState;
        let mut editor = source.clone();
        cfg.timetable.last_departure_ms = 0;
        let configure = |editor: &mut WorldEditor, turnaround_ms| {
            editor.configure_bus_operation(
                cfg.route_id.clone(),
                cfg.timetable.clone(),
                cfg.vehicle.clone(),
                cfg.assumed_speed_kmh,
                cfg.dwell_ms,
                turnaround_ms,
                &editor.operation_snapshot().timetable_edit_token,
            )
        };
        configure(&mut editor, None).unwrap();
        let old_spec = editor.world.compile_bus_operation(&cfg).unwrap();
        let expected_leg = 1; // A -> mid -> B, B -> mid -> A.
        let b = old_spec.legs[expected_leg].to.clone();
        assert_eq!(b, "bus:ofm-bus:B1:終点");
        let mut probe = editor.world.operation().unwrap().runtime.clone();
        probe.set_paused(false);
        let terminal_arrival = probe
            .advance_to(120_000, 100)
            .unwrap()
            .arrivals
            .into_iter()
            .find(|arrival| arrival.station_id == b)
            .unwrap();
        let key = terminal_arrival.train;
        let arrival_ms = terminal_arrival.at_ms;
        let folder = tempfile::tempdir().unwrap();
        let reopen = |instance: &mut WorldEditor, filename| {
            let check_file = folder.path().join(filename);
            instance.save(&check_file).unwrap();
            let mut reopened = WorldEditor::new();
            reopened.load(&check_file).unwrap();
            reopened
        };
        {
            let mut legacy = editor.clone();
            let runtime = &mut legacy.world.operation_mut().unwrap().runtime;
            runtime.set_paused(false);
            runtime.advance_to(arrival_ms + 2_000, 100).unwrap();
            runtime.set_paused(true);
            let old_save = legacy.world.operation_save().unwrap();
            assert_eq!(old_save.bus_configs[0].turnaround_ms, None);
            let serialized = serde_json::to_value(&old_save).unwrap();
            assert!(serialized["bus_configs"][0].get("turnaround_ms").is_none());
            assert_eq!(
                old_save.runtime.trains[0].state,
                TrainState::Waiting {
                    ready_ms: arrival_ms + cfg.dwell_ms
                },
            );
            let reopened = reopen(&mut legacy, "legacy.rosenfu");
            assert_eq!(reopened.world.operation_save().unwrap(), old_save);
            assert_eq!(
                rosenfu_storage::encode(&reopened.world).unwrap(),
                rosenfu_storage::encode(&legacy.world).unwrap(),
            );
        }

        assert!(configure(&mut editor, Some(3_600_001)).is_err());
        configure(&mut editor, Some(120_000)).unwrap();
        // An ordinary client/config edit omitting the new field neither clears
        // nor upgrades any already configured round trip.
        configure(&mut editor, None).unwrap();
        let current = editor.world.operation().unwrap();
        assert_eq!(current.bus_configs[0].turnaround_ms, Some(120_000));
        let mut expected = old_spec.clone();
        expected.legs[expected_leg].dwell_ms = 120_000;
        assert_eq!(
            editor
                .world
                .compile_bus_operation(&current.bus_configs[0])
                .unwrap(),
            expected,
        );
        let offers = current.runtime.passenger_offers();
        assert_eq!(offers[2].from, b);
        assert_eq!(offers[2].through_seconds, 120.);
        assert_eq!(offers[3].through_seconds, 3.);
        let new_board = editor
            .preview_fixed_od(b, expected.legs[2].to.clone(), Some(10.), None)
            .unwrap();
        assert_eq!(new_board.rail.len(), 1);
        assert_eq!(new_board.rail[0].time.ride_seconds, offers[2].ride_seconds);
        assert!((new_board.rail[0].trips - 10.).abs() < 1e-9);
        let runtime = &mut editor.world.operation_mut().unwrap().runtime;
        runtime.set_paused(false);
        runtime.advance_to(arrival_ms + 119_999, 100).unwrap();
        let waiting = runtime.trains().next().unwrap();
        assert_eq!(waiting.key, key);
        assert_eq!(waiting.leg, expected_leg + 1);
        assert_eq!(
            waiting.state,
            TrainState::Waiting {
                ready_ms: arrival_ms + 120_000
            }
        );
        assert_eq!(waiting.delay_ms, 0);
        runtime.set_paused(true);
        let saved = editor.world.operation_save().unwrap();
        let mut reopened = reopen(&mut editor, "new.rosenfu");
        assert_eq!(reopened.world.operation_save().unwrap(), saved);
        let runtime = &mut reopened.world.operation_mut().unwrap().runtime;
        runtime.set_paused(false);
        assert!(runtime
            .advance_to(arrival_ms + 120_000, 100)
            .unwrap()
            .arrivals
            .is_empty());
        let moving = runtime.trains().next().unwrap();
        assert_eq!(moving.key, key);
        assert!(matches!(moving.state, TrainState::Moving { entered_ms, .. }
            if entered_ms == arrival_ms + 120_000));
        assert_eq!(moving.delay_ms, 0);
    }

    #[test]
    fn manual_stop_uses_exact_occurrence_preserves_geometry_and_serves_both_or_one() {
        use rosenfu_core::ofm_evidence::{
            BusInteriorStop, BusPathBinding, BusPattern, BusVertex, BusVia, BUS_GRAPH_RULE,
        };
        use rosenfu_core::operation::{Timetable, Vehicle};
        let raw="Gj4KDnRyYW5zcG9ydGF0aW9ueAIogCASFxICAAAYAiIPCcgByAEa2AQA2AQA2AQAGgVjbGFzcyIHCgVtaW5vcg==";
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .unwrap();
        let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
        let tile = fixture()["tileBindings"][0].clone();
        let key = tile["canonical_url"].as_str().unwrap();
        let cache = || {
            vec![(
                OfmRawInput {
                    sha256: sha.clone(),
                    byte_length: bytes.len(),
                    base64: raw.into(),
                },
                OfmBindingInput {
                    key: key.into(),
                    url: key.into(),
                    raw_sha256: sha.clone(),
                    revision: tile["revision"].as_str().unwrap().into(),
                    z: 14,
                    x: tile["x"].as_u64().unwrap() as u32,
                    y: tile["y"].as_u64().unwrap() as u32,
                    decoder_rule: tile["decoder_rule"].as_str().unwrap().into(),
                    license: "ODbL-1.0".into(),
                    attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                },
            )]
        };
        let vertex = |n| BusVertex {
            feature: 0,
            line: 0,
            vertex: n,
        };
        let mut editor = WorldEditor::new();
        editor.rename_company("停留所の検証".into()).unwrap();
        let actual = BusPathBinding {
            rule: BUS_GRAPH_RULE.into(),
            grade_policy: None,
            tile_key: key.into(),
            tile_raw_sha256: sha.clone(),
            respect_oneway: false,
            start: vertex(0),
            end: vertex(2),
            additional_tiles: vec![],
            end_tile_key: None,
            pattern: Some(BusPattern::OpenRoundTrip),
            via: vec![],
            interior_stops: vec![],
        };
        let preview = |editor: &WorldEditor, p: BusPathBinding, cached| {
            editor
                .preview_gameplay_ofm_bus_details(
                    &editor.ofm_world_token().unwrap(),
                    "B1".into(),
                    "市内線".into(),
                    p,
                    cached,
                    GameplayBusPreviewOptions {
                        request_stop_targets: true,
                        ..Default::default()
                    },
                )
                .unwrap()
        };
        let stops = |id: &str, name: &str, target: &str| BusInteriorStop {
            id: id.into(),
            name: name.into(),
            occurrence_key: target.into(),
            fraction_q: 250_000,
        };
        let initial = editor.snapshot();
        let short_auto = editor
            .preview_gameplay_ofm_bus_details(
                &editor.ofm_world_token().unwrap(),
                "B1".into(),
                "市内線".into(),
                actual.clone(),
                cache(),
                GameplayBusPreviewOptions {
                    request_auto_stop_suggestions: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(short_auto.auto_stop_suggestions.is_empty());
        let baseline = preview(&editor, actual.clone(), cache());
        assert_eq!(
            baseline.stop_occurrence_keys.len(),
            baseline.points.len() - 1
        );
        assert!(baseline.interior_stops.is_empty());
        assert_eq!(editor.snapshot(), initial);
        let mut simple = actual.clone();
        simple
            .interior_stops
            .push(stops("centre", "中央", &baseline.stop_occurrence_keys[0]));
        let proposed = preview(&editor, simple.clone(), cache());
        assert_eq!(proposed.points, baseline.points);
        assert!(proposed.interior_stops[0].serves_return);
        let sw = editor
            .put_gameplay_ofm_bus(
                &editor.ofm_world_token().unwrap(),
                "B1".into(),
                "市内線".into(),
                simple,
                cache(),
            )
            .unwrap();
        let id = proposed.interior_stops[0].stop_id.clone();
        assert!(sw
            .road_transport
            .bus_stops
            .iter()
            .any(|s| s.id == id && s.position == 0.25));
        let token = editor.operation_snapshot().timetable_edit_token.clone();
        editor
            .configure_bus_operation(
                "B1".into(),
                Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 120000,
                    headway_ms: 120000,
                },
                Vehicle {
                    max_speed_kmh: 60.,
                    acceleration_mps2: 1.,
                    braking_mps2: 1.2,
                    length_m: 12.,
                },
                30.,
                3000,
                None,
                &token,
            )
            .unwrap();
        let operation = editor.world.operation().unwrap();
        let cfg = operation.bus_configs[0].clone();
        let full = editor.world.compile_bus_operation(&cfg).unwrap();
        assert_eq!(full.legs.len(), 4);
        assert_eq!(
            full.legs
                .iter()
                .filter(|leg| leg.to == format!("bus:{id}"))
                .count(),
            2
        );
        assert!(full.legs.iter().all(|leg| leg.sections[0].length_m > 0.));
        let mut runtime = operation.runtime.clone();
        runtime.set_paused(false);
        let arrived = runtime.advance_to(119_999, 1000).unwrap();
        assert_eq!(
            arrived
                .arrivals
                .iter()
                .filter(|a| a.station_id == format!("bus:{id}"))
                .count(),
            2
        );
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("interior.rosenfu");
        editor.save(&file).unwrap();
        let mut loaded = WorldEditor::new();
        loaded.load(&file).unwrap();
        assert_eq!(loaded.snapshot(), sw);
        assert_eq!(loaded.world.compile_bus_operation(&cfg).unwrap(), full);
        verify_ordinary_bus_turnaround(&loaded, cfg);

        // Two distinct visits to the same outward atom and physical offset.
        // Strict endpoint return traverses that location once and must not
        // silently choose which authored stop to serve.
        let mut looped = actual;
        looped.respect_oneway = true;
        looped.via = vec![
            BusVia {
                id: "out-first".into(),
                tile_key: key.into(),
                vertex: vertex(1),
            },
            BusVia {
                id: "back-through".into(),
                tile_key: key.into(),
                vertex: vertex(0),
            },
        ];
        let loop_preview = preview(&loaded, looped.clone(), vec![]);
        assert_eq!(loop_preview.points[0], loop_preview.points[2]);
        assert_ne!(
            loop_preview.stop_occurrence_keys[0],
            loop_preview.stop_occurrence_keys[2]
        );
        looped.interior_stops = vec![
            stops("first", "一周目", &loop_preview.stop_occurrence_keys[0]),
            stops("second", "二周目", &loop_preview.stop_occurrence_keys[2]),
        ];
        let loop_stops = preview(&loaded, looped.clone(), vec![]);
        assert_eq!(loop_stops.interior_stops.len(), 2);
        assert_eq!(
            loop_stops.interior_stops[0].point,
            loop_stops.interior_stops[1].point
        );
        assert_ne!(
            loop_stops.interior_stops[0].stop_id,
            loop_stops.interior_stops[1].stop_id
        );
        assert!(loop_stops.interior_stops.iter().all(|s| !s.serves_return));
        let saved_loop = loaded
            .put_gameplay_ofm_bus(
                &loaded.ofm_world_token().unwrap(),
                "B1".into(),
                "市内線".into(),
                looped.clone(),
                vec![],
            )
            .unwrap();

        let mut removed = looped.clone();
        removed.via.clear();
        let now = loaded.ofm_world_token().unwrap();
        let err = loaded
            .put_gameplay_ofm_bus(&now, "B1".into(), "市内線".into(), removed, vec![])
            .err()
            .unwrap();
        assert!(err.contains("置き直してください"));
        assert_eq!(loaded.snapshot(), saved_loop);

        // A different end on the same last leg redraws the route, not either
        // existing stop's exact atom/control occurrence.
        looped.end = vertex(3);
        let changed_preview = preview(&loaded, looped.clone(), vec![]);
        assert!(changed_preview.points.len() > loop_stops.points.len());
        assert_eq!(changed_preview.interior_stops, loop_stops.interior_stops);
        let changed = loaded
            .put_gameplay_ofm_bus(&now, "B1".into(), "延伸した市内線".into(), looped, vec![])
            .unwrap();
        assert!(changed
            .road_transport
            .bus_stops
            .iter()
            .any(|s| s.id == loop_stops.interior_stops[0].stop_id));
        assert_eq!(loaded.undo_world_edit(changed).unwrap(), saved_loop);
    }

    #[test]
    fn observed_bus_bridge_does_not_join_legacy_ground_at_equal_xy_after_reopen() {
        use rosenfu_core::ofm_evidence::{
            BusGradePolicy, BusPathBinding, BusPattern, BusVertex, BUS_GRAPH_RULE,
        };
        let encoded="GpgBCg50cmFuc3BvcnRhdGlvbngCKIAgEhESAgAAGAIiCQnIAcgBCpADABIYEgYAAAEBAgIYAiIMCdgEyAESrAIA9AMAEhISAgAAGAIiCgmEB1ASAHgAyAESERICAAAYAiIJCfgKyAEKkAMAGgVjbGFzcxoHYnJ1bm5lbBoFbGF5ZXIiBwoFbWlub3IiCAoGYnJpZGdlIgMKATE=";
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let sha = rosenfu_core::ofm_evidence::raw_sha256(&bytes);
        let tile = fixture()["tileBindings"][0].clone();
        let key = tile["canonical_url"].as_str().unwrap();
        let cache = vec![(
            OfmRawInput {
                sha256: sha.clone(),
                byte_length: bytes.len(),
                base64: encoded.into(),
            },
            OfmBindingInput {
                key: key.into(),
                url: key.into(),
                raw_sha256: sha.clone(),
                revision: tile["revision"].as_str().unwrap().into(),
                z: 14,
                x: tile["x"].as_u64().unwrap() as u32,
                y: tile["y"].as_u64().unwrap() as u32,
                decoder_rule: tile["decoder_rule"].as_str().unwrap().into(),
                license: "ODbL-1.0".into(),
                attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
            },
        )];
        let v = |feature, vertex| BusVertex {
            feature,
            line: 0,
            vertex,
        };
        let legacy = BusPathBinding {
            rule: BUS_GRAPH_RULE.into(),
            tile_key: key.into(),
            tile_raw_sha256: sha,
            respect_oneway: false,
            start: v(2, 0),
            end: v(2, 2),
            additional_tiles: vec![],
            end_tile_key: None,
            grade_policy: None,
            pattern: Some(BusPattern::OpenRoundTrip),
            via: vec![],
            interior_stops: vec![],
        };
        let mut editor = WorldEditor::new();
        editor.rename_company("立体交差の検証".into()).unwrap();
        let below = editor
            .put_gameplay_ofm_bus(
                &editor.ofm_world_token().unwrap(),
                "below".into(),
                "下の道路".into(),
                legacy.clone(),
                cache,
            )
            .unwrap();
        assert!(serde_json::to_value(&legacy)
            .unwrap()
            .get("gradePolicy")
            .is_none());
        let mut approach = legacy;
        approach.grade_policy = Some(BusGradePolicy::ObservedConnectors);
        approach.start = v(0, 0);
        approach.end = v(3, 1);
        let mut impossible = approach.clone();
        impossible.end = v(2, 2);
        let token = editor.ofm_world_token().unwrap();
        assert!(editor
            .put_gameplay_ofm_bus(
                &token,
                "above".into(),
                "橋の道路".into(),
                impossible,
                vec![]
            )
            .is_err());
        assert_eq!(editor.snapshot(), below);
        let both = editor
            .put_gameplay_ofm_bus(&token, "above".into(), "橋の道路".into(), approach, vec![])
            .unwrap();
        assert!(both
            .road_transport
            .bus_routes
            .contains(&below.road_transport.bus_routes[0]));
        for edge in &below.road_transport.edges {
            assert!(both.road_transport.edges.contains(edge));
        }
        let route = both
            .road_transport
            .bus_routes
            .iter()
            .find(|r| r.id == "above")
            .unwrap();
        let road = |id: &str| {
            both.road_transport
                .edges
                .iter()
                .find(|e| e.id == id)
                .unwrap()
        };
        let elevated = road(&route.edges[1]);
        let flat = road(&below.road_transport.bus_routes[0].edges[0]);
        assert_eq!(elevated.level, None); // source grade is not an invented metre level
        assert_eq!(
            flat.level,
            Some(rosenfu_core::world::InfrastructureLevel::Ground)
        );
        assert_eq!(elevated.geometry[1], flat.geometry[1]);
        assert_ne!(elevated.to, flat.to);
        let dir = tempfile::tempdir().unwrap();
        let save = dir.path().join("grade-bridge.rosenfu");
        editor.save(&save).unwrap();
        let mut reopened = WorldEditor::new();
        reopened.load(&save).unwrap();
        assert_eq!(reopened.snapshot(), both);
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
