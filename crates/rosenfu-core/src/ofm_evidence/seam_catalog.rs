//! Save-replay trust boundary: serialized recipes are references, never evidence.
use super::{
    raw_sha256, replay_westbound_seam_traversal, Error, EvidenceStore, Result,
    WestboundSeamReceipt, WestboundSeamTraversal, WESTBOUND_SEAM_WITNESS_RULE,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

const INSTALLED: &[u8] = include_bytes!("installed_westbound_seams.json");
const MAX_MANIFEST_BYTES: usize = 128 * 1024;
const MAX_ENTRIES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledWestboundSeamRecipe {
    pub version: u16,
    /// An opaque immutable reference, not a self-certified digest.
    pub catalog_key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestV1 {
    version: u16,
    receipts: Vec<WestboundSeamReceipt>,
}
struct ReceiptEntry {
    digest: String,
    bytes: Vec<u8>,
    keys: [String; 2],
}
struct Catalog(BTreeMap<String, ReceiptEntry>);

impl Catalog {
    fn load(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(Error::Limit);
        }
        let manifest: ManifestV1 = serde_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
        if manifest.version != 1 {
            return Err(Error::Unsupported);
        }
        if manifest.receipts.len() > MAX_ENTRIES {
            return Err(Error::Limit);
        }
        let mut entries = BTreeMap::new();
        for receipt in manifest.receipts {
            if receipt.rule != WESTBOUND_SEAM_WITNESS_RULE {
                return Err(Error::Unsupported);
            }
            let [west, east] = &receipt.tiles_west_east;
            west.validate()?;
            east.validate()?;
            if east.x != west.x + 1
                || west.y != east.y
                || west.revision != east.revision
                || receipt.raw_feature_id == 0
                || receipt.vertices_west_east.contains(&0)
            {
                return Err(Error::Binding);
            }
            // The v1 wire fields have a fixed order and contain no maps. A
            // different canonicalization must use a new recipe/catalog version.
            let bytes = serde_json::to_vec(&receipt).map_err(|_| Error::Malformed)?;
            let digest = raw_sha256(&bytes);
            let keys = receipt.tiles_west_east.map(|tile| tile.canonical_url);
            if entries
                .insert(
                    format!("ofm-westbound-v1:{digest}"),
                    ReceiptEntry {
                        digest,
                        bytes,
                        keys,
                    },
                )
                .is_some()
            {
                return Err(Error::Binding);
            }
        }
        Ok(Self(entries))
    }

    fn replay(
        &self,
        archive: &EvidenceStore,
        recipe: &InstalledWestboundSeamRecipe,
    ) -> Result<WestboundSeamTraversal> {
        if recipe.version != 1 {
            return Err(Error::Unsupported);
        }
        // This exact length also prevents an unbounded lookup key in the
        // resolver. The world/save parser separately owns total byte limits.
        if recipe.catalog_key.len() != "ofm-westbound-v1:".len() + 64 {
            return Err(Error::Binding);
        }
        let entry = self.0.get(&recipe.catalog_key).ok_or(Error::Binding)?;
        // Authoritative tiles come from the installed receipt, not the save.
        let scoped = archive.scope(&entry.keys)?;
        replay_westbound_seam_traversal(&scoped, &entry.bytes, &entry.digest)
    }
}

/// Rebuild a saved reference using the release-baked catalog and the matching
/// retained archive. Missing entries, changed tile bindings, or unsupported
/// recipes fail closed. No caller-provided digest, network, or alternate
/// manifest can grant authority through this entry point.
pub fn replay_installed_westbound_seam(
    archive: &EvidenceStore,
    recipe: &InstalledWestboundSeamRecipe,
) -> Result<WestboundSeamTraversal> {
    #[cfg(test)]
    if let Some(result) = TEST_CATALOG.with(|catalog| {
        catalog
            .borrow()
            .as_ref()
            .map(|catalog| catalog.replay(archive, recipe))
    }) {
        return result;
    }
    static CATALOG: OnceLock<Result<Catalog>> = OnceLock::new();
    match CATALOG.get_or_init(|| Catalog::load(INSTALLED)) {
        Ok(catalog) => catalog.replay(archive, recipe),
        Err(error) => Err(*error),
    }
}

#[cfg(test)]
thread_local! {
    static TEST_CATALOG: std::cell::RefCell<Option<Catalog>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ofm_evidence::{RawEvidence, TileBinding, DECODER_RULE};

    fn var(mut n: u64, output: &mut Vec<u8>) {
        while n >= 128 {
            output.push((n as u8) | 128);
            n >>= 7;
        }
        output.push(n as u8);
    }
    fn words(data: &[u64]) -> Vec<u8> {
        let mut output = vec![];
        for &n in data {
            var(n, &mut output);
        }
        output
    }
    fn field(tag: u64, body: &[u8], output: &mut Vec<u8>) {
        var(tag * 8 + 2, output);
        var(body.len() as u64, output);
        output.extend_from_slice(body);
    }

    fn tile(points: &[[i32; 2]]) -> Vec<u8> {
        let zig = |n: i32| ((n << 1) ^ (n >> 31)) as u64;
        let mut geom = vec![
            9,
            zig(points[0][0]),
            zig(points[0][1]),
            ((points.len() - 1) as u64) * 8 + 2,
        ];
        for p in points.windows(2) {
            geom.extend([zig(p[1][0] - p[0][0]), zig(p[1][1] - p[0][1])]);
        }
        let mut feature = words(&[8, 21, 24, 2]); // ID and LineString type
        field(2, &[0, 0, 1, 1], &mut feature);
        field(4, &words(&geom), &mut feature);
        let mut layer = words(&[120, 2, 40, 4096]); // version and extent
        field(1, b"transportation", &mut layer);
        field(2, &feature, &mut layer);
        field(3, b"class", &mut layer);
        field(3, b"oneway", &mut layer);
        let mut class = vec![];
        field(1, b"secondary", &mut class);
        field(4, &class, &mut layer);
        field(4, &words(&[40, 1]), &mut layer);
        let mut out = vec![];
        field(3, &layer, &mut out);
        out
    }

    fn fixture(extra_east_point: bool) -> (WestboundSeamReceipt, EvidenceStore) {
        let west = tile(&[[4176, 1800], [4156, 1790], [3856, 1640]]);
        let mut east = vec![[180, 1850], [80, 1800], [60, 1790], [-1385, 1067]];
        if extra_east_point {
            east.push([-1393, 1065]);
        }
        let east = tile(&east);
        let revision = "synthetic_receipt_v1";
        let raws = [west, east];
        let tiles = std::array::from_fn(|i| {
            let x = 14552 + i as u32;
            TileBinding {
                canonical_url: format!(
                    "https://tiles.openfreemap.org/planet/{revision}/14/{x}/6451.pbf"
                ),
                revision: revision.to_owned(),
                z: 14,
                x,
                y: 6451,
                raw_sha256: raw_sha256(&raws[i]),
                decoder_rule: DECODER_RULE.to_owned(),
            }
        });
        let entries = raws
            .into_iter()
            .zip(&tiles)
            .map(|(bytes, tile)| RawEvidence {
                sha256: tile.raw_sha256.clone(),
                bytes,
            })
            .collect();
        let archive = EvidenceStore::from_archive_parts(entries, tiles.to_vec()).unwrap();
        (
            WestboundSeamReceipt {
                rule: WESTBOUND_SEAM_WITNESS_RULE.into(),
                tiles_west_east: tiles,
                raw_feature_id: 21,
                vertices_west_east: [1, 2],
            },
            archive,
        )
    }

    #[test]
    fn stored_recipe_uses_only_release_catalog_and_exact_retained_archive() {
        let (receipt, archive) = fixture(false);
        let make_manifest = |receipt: &WestboundSeamReceipt| {
            serde_json::to_vec(&serde_json::json!({"version":1,"receipts":[receipt]})).unwrap()
        };
        let catalog = Catalog::load(&make_manifest(&receipt)).unwrap();
        let recipe = InstalledWestboundSeamRecipe {
            version: 1,
            catalog_key: catalog.0.keys().next().unwrap().clone(),
        };
        let bytes = serde_json::to_vec(&recipe).unwrap();
        let reloaded: InstalledWestboundSeamRecipe = serde_json::from_slice(&bytes).unwrap();
        let positive = catalog.replay(&archive, &reloaded).unwrap();
        assert_eq!(
            positive.grid_points(),
            &[[80, 1800], [60, 1790], [-240, 1640]]
        );
        assert_eq!(catalog.replay(&archive, &recipe).unwrap(), positive);
        assert_eq!(
            replay_installed_westbound_seam(&archive, &reloaded),
            Err(Error::Binding)
        );
        let (new_receipt, changed) = fixture(true);
        assert_eq!(catalog.replay(&changed, &reloaded), Err(Error::Binding));
        let new_catalog = Catalog::load(&make_manifest(&new_receipt)).unwrap();
        assert_eq!(new_catalog.replay(&archive, &reloaded), Err(Error::Binding));
        assert_eq!(
            catalog.replay(&EvidenceStore::default(), &reloaded),
            Err(Error::Binding)
        );
        assert!(serde_json::from_slice::<InstalledWestboundSeamRecipe>(
            br#"{"version":1,"catalogKey":"fake","digest":"inline"}"#
        )
        .is_err());
        assert_eq!(
            catalog.replay(
                &archive,
                &InstalledWestboundSeamRecipe {
                    version: 2,
                    ..recipe
                }
            ),
            Err(Error::Unsupported)
        );
        let duplicates =
            serde_json::to_vec(&serde_json::json!({"version":1,"receipts":[&receipt,&receipt]}))
                .unwrap();
        assert!(matches!(Catalog::load(&duplicates), Err(Error::Binding)));
    }

    #[test]
    fn installed_recipe_admits_exact_world_and_restarts_bound_operation() {
        use crate::operation::{Timetable, TrainState, Vehicle};
        use crate::world::{BusOperationConfig, World, WorldDraft};

        let (receipt, archive) = fixture(false);
        let bytes =
            serde_json::to_vec(&serde_json::json!({"version":1,"receipts":[receipt]})).unwrap();
        let catalog = Catalog::load(&bytes).unwrap();
        let recipe = InstalledWestboundSeamRecipe {
            version: 1,
            catalog_key: catalog.0.keys().next().unwrap().clone(),
        };
        let old = TEST_CATALOG.with(|slot| slot.replace(Some(catalog)));
        struct Reset(Option<Catalog>);
        impl Drop for Reset {
            fn drop(&mut self) {
                TEST_CATALOG.with(|slot| {
                    slot.replace(self.0.take());
                });
            }
        }
        let _reset = Reset(old);
        let mut world = World::default();
        let player = world.add_player("player".into()).unwrap();
        let owner = world.add_company(player, "operator".into()).unwrap();
        let original = world.snapshot().clone();
        assert!(world
            .candidate_with_installed_seam_bus(owner, "bus".into(), "Bus".into(), recipe.clone())
            .is_err());
        assert_eq!(world.snapshot(), &original);
        let with_archive =
            World::new(original.with_road_evidence(archive.clone()).unwrap()).unwrap();
        let east = archive
            .bindings()
            .find(|t| t.x == 14553)
            .unwrap()
            .canonical_url
            .clone();
        let east_store = archive.scope(std::slice::from_ref(&east)).unwrap();
        let vertex = |vertex| crate::ofm_evidence::BusVertex {
            feature: 0,
            line: 0,
            vertex,
        };
        let path = crate::ofm_evidence::find_gameplay_bus_path(
            &east_store,
            &east,
            vertex(0),
            vertex(1),
            true,
        )
        .unwrap()
        .unwrap();
        let former = with_archive
            .candidate_with_gameplay_ofm_bus(
                owner,
                "legacy".into(),
                "Legacy".into(),
                path.binding,
                vec![],
            )
            .unwrap();
        let legacy_before = former.snapshot().clone();
        assert!(former
            .candidate_with_installed_seam_bus(owner, "bus".into(), "Bus".into(), recipe.clone())
            .is_err());
        assert_eq!(former.snapshot(), &legacy_before);
        let mut candidate = with_archive
            .candidate_with_installed_seam_bus(owner, "bus".into(), "Bus".into(), recipe)
            .unwrap();
        let roads = &candidate.snapshot().road_transport;
        assert_eq!(
            (
                roads.nodes.len(),
                roads.edges.len(),
                roads.bus_routes.len(),
                roads.bus_stops.len()
            ),
            (3, 2, 1, 2)
        );
        assert!(with_archive.snapshot().road_transport.bus_routes.is_empty());

        let good = serde_json::to_vec(candidate.snapshot()).unwrap();
        let mut bad = candidate.snapshot().clone();
        bad.road_transport.edges[0].id.push_str("-tampered");
        assert!(World::from_saved_draft(bad, archive.clone()).is_err());
        let mut reopened = World::from_saved_draft(
            serde_json::from_slice::<WorldDraft>(&good).unwrap(),
            archive,
        )
        .unwrap();
        assert_eq!(serde_json::to_vec(reopened.snapshot()).unwrap(), good);

        let config = BusOperationConfig {
            route_id: "bus".into(),
            timetable: Timetable {
                first_departure_ms: 0,
                last_departure_ms: 1000,
                headway_ms: 1000,
            },
            vehicle: Vehicle {
                max_speed_kmh: 35.,
                acceleration_mps2: 1.,
                braking_mps2: 1.,
                length_m: 12.,
            },
            assumed_speed_kmh: 35.,
            dwell_ms: 0,
            turnaround_ms: None,
        };
        candidate
            .configure_all_operation(vec![], vec![config])
            .unwrap();
        let runtime = &mut candidate.operation_mut().unwrap().runtime;
        runtime.advance_to(1001, 20).unwrap();
        let mut trains = runtime.trains();
        assert!(matches!(
            trains.next().unwrap().state,
            TrainState::Moving { .. }
        ));
        assert!(matches!(
            trains.next().unwrap().state,
            TrainState::Waiting { .. }
        ));
        drop(trains);
        let checkpoint = candidate.operation_save().unwrap();
        assert_eq!(
            checkpoint.runtime.services[0].legs[0].sections[0]
                .resources
                .iter()
                .filter(|k| k.starts_with("ofm-bus:fragment-lock:"))
                .count(),
            2
        );
        let mut broken = checkpoint.clone();
        broken.runtime.services[0].legs[0].sections[0]
            .resources
            .pop();
        assert!(reopened.restore_operation(Some(broken)).is_err());
        let mut invalidated = checkpoint.clone();
        invalidated.invalidated = true;
        assert!(reopened.restore_operation(Some(invalidated)).is_err());
        assert!(reopened.operation().is_none());
        reopened
            .restore_operation(Some(
                serde_json::from_slice(&serde_json::to_vec(&checkpoint).unwrap()).unwrap(),
            ))
            .unwrap();
        assert_eq!(reopened.operation_save(), Some(checkpoint));
        let advance = |w: &mut World| {
            w.operation_mut()
                .unwrap()
                .runtime
                .advance_to(120_000, 50)
                .unwrap()
        };
        let result = advance(&mut candidate);
        assert_eq!(result, advance(&mut reopened));
        assert_eq!(result.arrivals.len(), 2);
        assert!(result.arrivals[1].delay_ms > 0);
    }
}
