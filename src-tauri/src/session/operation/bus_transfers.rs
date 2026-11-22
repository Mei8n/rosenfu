//! Fixed OD only: connect coincident, served stops on the same verified directed ground atom.
//! A nearby point, indistinguishable screen position or opposite edge is not a walking link.
use rosenfu_core::demand::{PassengerLeg, WalkLink};
use rosenfu_core::road_transport::{Access, BusStopLocationKind};
use rosenfu_core::world::{InfrastructureLevel, WorldDraft};
use std::collections::{BTreeMap, BTreeSet};

const LINK_LIMIT: usize = 512;

pub(super) fn coincident_ordinary_bus_walks(
    world: &WorldDraft,
    legs: &[PassengerLeg],
) -> Result<Vec<WalkLink>, String> {
    let roads = &world.road_transport;
    if world.ofm_gameplay_bus_routes.len() < 2 {
        return Ok(vec![]);
    }
    let mut served = BTreeMap::<&str, BTreeSet<&str>>::new();
    for leg in legs {
        let Some(route) = leg.service_id.strip_prefix("bus:") else {
            continue;
        };
        if !world.ofm_gameplay_bus_routes.contains_key(route) {
            continue;
        }
        let stops = served.entry(route).or_default();
        if leg.board_allowed {
            stops.extend(leg.from.strip_prefix("bus:"));
        }
        if leg.alight_allowed {
            stops.extend(leg.to.strip_prefix("bus:"));
        }
    }
    if served.len() < 2 {
        return Ok(vec![]);
    }
    let stops: BTreeMap<_, _> = roads.bus_stops.iter().map(|s| (s.id.as_str(), s)).collect();
    let edges: BTreeMap<_, _> = roads.edges.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut coincident = BTreeMap::<_, BTreeSet<(&str, &str)>>::new();
    for route in &roads.bus_routes {
        let Some(served_stops) = served.get(route.id.as_str()) else {
            continue;
        };
        for occurrence in &route.stops {
            let Some(stop) = stops.get(occurrence.stop_id.as_str()) else {
                continue;
            };
            if !served_stops.contains(stop.id.as_str())
                || stop.location_kind != BusStopLocationKind::PlayerCreated
                || route.edges.get(occurrence.edge_index) != Some(&stop.edge_id)
                || !(0.0..=1.0).contains(&stop.position)
            {
                continue;
            }
            let Some(edge) = edges.get(stop.edge_id.as_str()) else {
                continue;
            };
            if edge.source != "openfreemap_gameplay_bus"
                || edge.bus_access != Access::Allowed
                || edge.level != Some(InfrastructureLevel::Ground)
            {
                continue;
            }
            // The committed edge ID is bound to the raw tile digest, fragment, ordinal
            // and directed clip. Do not round a nearby fraction into the same stop.
            let fraction = if stop.position == 0.0 {
                0
            } else {
                stop.position.to_bits()
            };
            coincident
                .entry((edge.id.as_str(), edge.revision.as_str(), fraction))
                .or_default()
                .insert((route.id.as_str(), stop.id.as_str()));
        }
    }
    let mut pairs = BTreeSet::new();
    for group in coincident.values() {
        // One distinct call identity per service. If a service revisits this
        // physical position with different stop IDs, do not infer which
        // occurrence should be the interchange.
        let mut services = BTreeMap::new();
        for &(route, stop) in group {
            services
                .entry(route)
                .and_modify(|slot| *slot = None)
                .or_insert(Some(stop));
        }
        // A deterministic bidirectional star has exactly one simple no-walk
        // path between any two aliases (a clique would count duplicates).
        let mut stops = services.into_values().flatten();
        let Some(hub) = stops.next() else {
            continue;
        };
        for candidate in stops {
            if hub == candidate {
                continue;
            }
            pairs.insert((hub, candidate));
            pairs.insert((candidate, hub));
            if pairs.len() > LINK_LIMIT {
                return Err("同じ乗り場でのバス乗り換えが試算上限を超えました。".into());
            }
        }
    }
    Ok(pairs
        .into_iter()
        .map(|(from, to)| WalkLink {
            from: format!("bus:{from}"),
            to: format!("bus:{to}"),
            seconds: 0.0,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{OfmBindingInput, OfmRawInput, WorldEditor};
    use base64::Engine;
    use rosenfu_core::ofm_evidence::{BusInteriorStop, BusPathBinding, BusVertex, BUS_GRAPH_RULE};
    use rosenfu_core::operation::{Timetable, Vehicle};

    // Public-format synthetic MVT, two ground minor roads sharing a T-junction.
    // A→west and B→west arrive from different branches, share one directed atom.
    const RAW: &str = "GlEKDnRyYW5zcG9ydGF0aW9ueAIogCASFBICAAAYAiIMCcgByAESyAEAyAEAEhQSAgAAGAIiDAmQA8gBEgDIAQDIARoFY2xhc3MiBwoFbWlub3I=";
    const URL: &str = "https://tiles.openfreemap.org/planet/20261101_080013_pt/14/14552/6451.pbf";

    fn ordinary_services(third: bool) -> WorldEditor {
        let mut ed = WorldEditor::new();
        ed.rename_company("ordinary transfer test".into()).unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(RAW)
            .unwrap();
        let sha = rosenfu_core::ofm_evidence::raw_sha256(&raw);
        let cache = || {
            vec![(
                OfmRawInput {
                    sha256: sha.clone(),
                    byte_length: raw.len(),
                    base64: RAW.into(),
                },
                OfmBindingInput {
                    key: URL.into(),
                    url: URL.into(),
                    raw_sha256: sha.clone(),
                    revision: "20261101_080013_pt".into(),
                    z: 14,
                    x: 14552,
                    y: 6451,
                    decoder_rule: "ofm_transportation_z14_v1".into(),
                    license: "ODbL-1.0".into(),
                    attribution: "© OpenMapTiles © OpenStreetMap contributors".into(),
                },
            )]
        };
        let routes = [(0, 2, "A"), (1, 2, "B"), (0, 1, "0")];
        for &(n, start, name) in &routes[..if third { 3 } else { 2 }] {
            let token = ed.ofm_world_token().unwrap();
            let mut binding = BusPathBinding {
                rule: BUS_GRAPH_RULE.into(),
                grade_policy: None,
                tile_key: URL.into(),
                tile_raw_sha256: sha.clone(),
                respect_oneway: false,
                start: BusVertex {
                    feature: n,
                    line: 0,
                    vertex: start,
                },
                end: BusVertex {
                    feature: 0,
                    line: 0,
                    vertex: 0,
                },
                additional_tiles: vec![],
                end_tile_key: None,
                pattern: None,
                interior_stops: vec![],
                via: vec![],
            };
            let preview = ed
                .preview_gameplay_ofm_bus_details(
                    &token,
                    name.into(),
                    name.into(),
                    binding.clone(),
                    cache(),
                    crate::session::GameplayBusPreviewOptions {
                        request_stop_targets: true,
                        request_auto_stop_suggestions: false,
                    },
                )
                .unwrap();
            let common = preview.stop_occurrence_keys.last().unwrap().clone();
            let stop = |id: &str, fraction_q| BusInteriorStop {
                id: id.into(),
                name: id.into(),
                occurrence_key: common.clone(),
                fraction_q,
            };
            binding.interior_stops.push(stop("shared", 500_000));
            if name == "B" {
                binding.interior_stops.push(stop("destination", 750_000));
            }
            ed.put_gameplay_ofm_bus(&token, name.into(), name.into(), binding, cache())
                .unwrap();
            ed.configure_bus_operation(
                name.into(),
                Timetable {
                    first_departure_ms: 0,
                    last_departure_ms: 60_000,
                    headway_ms: 60_000,
                },
                Vehicle {
                    max_speed_kmh: 40.0,
                    acceleration_mps2: 1.0,
                    braking_mps2: 1.2,
                    length_m: 12.0,
                },
                25.0,
                10_000,
                None,
                &ed.operation_edit_token(),
            )
            .unwrap();
        }
        ed
    }

    fn legs(ed: &WorldEditor) -> Vec<PassengerLeg> {
        ed.world
            .operation()
            .unwrap()
            .runtime
            .passenger_offers()
            .into_iter()
            .map(|l| PassengerLeg {
                leg_id: l.leg_id,
                sequence: l.sequence,
                board_allowed: l.board_allowed,
                alight_allowed: l.alight_allowed,
                service_id: l.service_id,
                from: l.from,
                to: l.to,
                ride_seconds: l.ride_seconds,
                through_seconds: l.through_seconds,
                headway_seconds: l.headway_seconds,
            })
            .collect()
    }

    #[test]
    fn same_native_directed_atom_transfers_once_and_reopens_without_saved_links() {
        let ed = ordinary_services(false);
        let snapshot = ed.snapshot();
        let roads = &snapshot.road_transport;
        let origin = format!("bus:{}", roads.bus_routes[0].stops[0].stop_id);
        let destination = format!("bus:{}", roads.bus_routes[1].stops[2].stop_id);
        let check = |ed: &WorldEditor| {
            let before = rosenfu_storage::encode(&ed.world).unwrap();
            let now = ed
                .preview_fixed_od(origin.clone(), destination.clone(), Some(17.0), None)
                .unwrap();
            assert!(now.search_complete);
            assert_eq!(
                now.rail.len(),
                1,
                "avoid zero-walk alias cycles and duplicate alternatives"
            );
            let path = &now.rail[0];
            assert_eq!(path.services, ["bus:A", "bus:B"]);
            assert_eq!(path.time.walk_seconds, 0.0);
            assert_eq!(path.time.transfers, 1);
            assert_eq!(path.trips, 17.0);
            assert_eq!(rosenfu_storage::encode(&ed.world).unwrap(), before);
        };
        check(&ed);
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("ordinary-transfer.rosenfu");
        ed.clone().save(&path).unwrap();
        let mut reopened = WorldEditor::new();
        reopened.load(&path).unwrap();
        check(&reopened);
        reopened
            .remove_gameplay_ofm_bus(&reopened.ofm_world_token().unwrap(), "B")
            .unwrap();
        assert!(
            coincident_ordinary_bus_walks(&reopened.snapshot(), &legs(&ed))
                .unwrap()
                .is_empty()
        );
        assert!(reopened
            .preview_fixed_od(origin, destination, Some(17.), None)
            .is_err());
    }

    #[test]
    fn three_services_share_one_zero_walk_star_without_duplicate_choices() {
        let ed = ordinary_services(true);
        let world = ed.snapshot();
        let by_id = |id| {
            world
                .road_transport
                .bus_routes
                .iter()
                .find(|r| r.id == id)
                .unwrap()
        };
        let [hub, a, b] = ["0", "A", "B"].map(|id| format!("bus:{}", by_id(id).stops[1].stop_id));
        let links = coincident_ordinary_bus_walks(&world, &legs(&ed)).unwrap();
        let at_hub = |other: &str| {
            links
                .iter()
                .filter(|l| (l.from == hub && l.to == other) || (l.from == other && l.to == hub))
                .count()
        };
        assert_eq!(at_hub(&a), 2);
        assert_eq!(at_hub(&b), 2);
        assert!(!links
            .iter()
            .any(|l| l.from == a && l.to == b || l.from == b && l.to == a));
        let assignment = ed
            .preview_fixed_od(
                format!("bus:{}", by_id("A").stops[0].stop_id),
                format!("bus:{}", by_id("B").stops[2].stop_id),
                Some(17.0),
                None,
            )
            .unwrap();
        assert!(assignment.search_complete);
        assert_eq!(assignment.rail.len(), 1);
        assert_eq!(assignment.rail[0].services, ["bus:A", "bus:B"]);
        assert_eq!(assignment.rail[0].time.transfers, 1);
        assert_eq!(assignment.rail[0].time.walk_seconds, 0.0);
        assert_eq!(assignment.rail[0].trips, 17.0);
    }

    #[test]
    fn different_revision_atom_grade_and_fraction_never_join_even_in_a_candidate() {
        let ed = ordinary_services(false);
        let world = ed.snapshot();
        let offers = legs(&ed);
        let stops = &world.road_transport.bus_routes[1].stops;
        let shared = &stops[1];
        let id = &shared.stop_id;
        let edge_id = &world
            .road_transport
            .bus_stops
            .iter()
            .find(|s| &s.id == id)
            .unwrap()
            .edge_id;
        let shared_offer = |links: Vec<WalkLink>| {
            links
                .iter()
                .any(|l| l.from == format!("bus:{id}") || l.to == format!("bus:{id}"))
        };
        assert!(shared_offer(
            coincident_ordinary_bus_walks(&world, &offers).unwrap()
        ));

        for change in 0..4 {
            let mut altered = world.clone();
            let graph = &mut altered.road_transport;
            let mut distinct = graph
                .edges
                .iter()
                .find(|e| &e.id == edge_id)
                .unwrap()
                .clone();
            distinct.id = format!("{edge_id}:different");
            match change {
                0 => distinct.revision = "different-authenticated-revision".into(),
                1 => distinct.level = Some(InfrastructureLevel::Elevated),
                2 => distinct.level = None,
                _ => {}
            }
            // A new source atom ID, including an independent revision or an
            // opposite carriageway, can have exactly the same screen geometry.
            graph.bus_routes[1].edges[shared.edge_index] = distinct.id.clone();
            graph
                .bus_stops
                .iter_mut()
                .find(|s| &s.id == id)
                .unwrap()
                .edge_id = distinct.id.clone();
            graph.edges.push(distinct);
            assert!(!shared_offer(
                coincident_ordinary_bus_walks(&altered, &offers).unwrap()
            ));
        }
        let mut moved = world.clone();
        moved
            .road_transport
            .bus_stops
            .iter_mut()
            .find(|s| &s.id == id)
            .unwrap()
            .position = 0.500001;
        assert!(!shared_offer(
            coincident_ordinary_bus_walks(&moved, &offers).unwrap()
        ));
        let mut elevated = world.clone();
        elevated
            .road_transport
            .edges
            .iter_mut()
            .find(|e| &e.id == edge_id)
            .unwrap()
            .level = None;
        assert!(!shared_offer(
            coincident_ordinary_bus_walks(&elevated, &offers).unwrap()
        ));
        assert!(coincident_ordinary_bus_walks(
            &world,
            &offers
                .iter()
                .filter(|l| l.service_id == "bus:A")
                .cloned()
                .collect::<Vec<_>>()
        )
        .unwrap()
        .is_empty());
    }
}
