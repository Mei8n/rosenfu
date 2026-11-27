//! In-memory proof for authored ground Tram over a simple chain of OFM cells.
//! Geometry proof only; it grants no traffic authority or acquisition policy.
use super::{
    adjacent_tram_anchor, build_observed_road_graph, distance, BusPathBinding, BusTileProof,
    BusVertex, Cell, Error, EvidenceStore, Result, SearchScratch, TileLine, BUS_RECTANGLE_RULE,
    MAX_MULTI_ATOMS, MAX_MULTI_RELAXATIONS,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_CHAIN_TILES: usize = 4;
const FRACTION_DENOMINATOR: i64 = 1_000_000;

/// Every tile is independently bound to its decoded bytes and is listed in
/// traversal order, including both endpoint cells. No legacy schema accepts it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthoredTramChainGuide {
    pub tiles: Vec<BusTileProof>,
    pub start: BusVertex,
    pub end: BusVertex,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_fraction_q: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_fraction_q: Option<u32>,
}

impl AuthoredTramChainGuide {
    pub(crate) fn evidence_request(&self) -> Result<BusPathBinding> {
        if !(3..=MAX_CHAIN_TILES).contains(&self.tiles.len()) {
            return Err(Error::Limit);
        }
        let first = &self.tiles[0];
        let last = &self.tiles[self.tiles.len() - 1];
        Ok(BusPathBinding {
            // Internal evidence scope only: this is not a saved Bus route.
            rule: BUS_RECTANGLE_RULE.into(),
            grade_policy: None,
            tile_key: first.tile_key.clone(),
            tile_raw_sha256: first.tile_raw_sha256.clone(),
            respect_oneway: false,
            start: self.start,
            end: self.end,
            same_atom_stops: None,
            fractional_terminals: None,
            additional_tiles: self.tiles[1..].to_vec(),
            end_tile_key: Some(last.tile_key.clone()),
            pattern: None,
            interior_stops: vec![],
            via: vec![],
        })
    }
}

/// Verify a three- or four-cell chain, then reconstruct only forward through
/// the uniquely matched seams. The route may turn within a cell; diagonal
/// hops, holes, repeated cells, nonconsecutive contacts, mixed revisions and
/// ambiguous seam links are not interpreted as valid transitions.
pub fn reconstruct_authored_tram_chain(
    store: &EvidenceStore,
    guide: &AuthoredTramChainGuide,
) -> Result<Option<Vec<[f64; 2]>>> {
    let request = guide.evidence_request()?;
    let first = &guide.tiles[0];
    let last = &guide.tiles[guide.tiles.len() - 1];
    let scope = store.scope(&request.tile_keys()?)?;
    let first_tile = scope.decoded(&first.tile_key)?;
    let last_tile = scope.decoded(&last.tile_key)?;
    let mut order = BTreeMap::<Cell, usize>::new();
    for (rank, proof) in guide.tiles.iter().enumerate() {
        let tile = &scope.decoded(&proof.tile_key)?.binding;
        if tile.raw_sha256 != proof.tile_raw_sha256 {
            return Err(Error::Hash);
        }
        if tile.revision != first_tile.binding.revision {
            return Err(Error::Binding);
        }
        let cell = Cell(tile.x, tile.y);
        for (&previous, &previous_rank) in &order {
            let distance = cell.0.abs_diff(previous.0) + cell.1.abs_diff(previous.1);
            if distance == 0 || (distance == 1) != (previous_rank + 1 == rank) {
                return Err(Error::Binding);
            }
        }
        order.insert(cell, rank);
    }
    if !strict_interior(&first_tile.lines, guide.start, guide.start_fraction_q)
        || !strict_interior(&last_tile.lines, guide.end, guide.end_fraction_q)
    {
        return Ok(None);
    }
    let graph = build_observed_road_graph(&scope, &request, true, Some(&order))?;
    let Some((start, from)) =
        adjacent_tram_anchor(&graph, &first.tile_key, guide.start, guide.start_fraction_q)
    else {
        return Ok(None);
    };
    let Some((end, to)) =
        adjacent_tram_anchor(&graph, &last.tile_key, guide.end, guide.end_fraction_q)
    else {
        return Ok(None);
    };

    let mut scratch = SearchScratch::new(graph.geometry.len());
    let mut budget = MAX_MULTI_RELAXATIONS;
    let mut best = None::<(f64, Vec<[f64; 2]>)>;
    for &from in &from {
        for &to in &to {
            let mut legs = Vec::new();
            if !scratch.leg(&graph, from, to, &mut budget, &mut legs)? {
                continue;
            }
            if legs.len() > MAX_MULTI_ATOMS - 3 {
                return Err(Error::Limit);
            }
            let mut points = Vec::with_capacity(legs.len() + 3);
            points.extend([start, graph.geometry[from].point()]);
            points.extend(legs.iter().map(|arc| graph.geometry[arc.to].point()));
            points.push(end);
            points.dedup();
            let length = points.windows(2).map(|p| distance(p[0], p[1])).sum();
            if best.as_ref().is_none_or(|(shortest, _)| length < *shortest) {
                best = Some((length, points));
            }
        }
    }
    Ok(best.map(|(_, points)| points))
}

// Whole-cell ownership checked exactly at the authored pick, before attempting
// the path. A buffered MVT duplicate or the seam line itself is never a pick.
fn strict_interior(lines: &[TileLine], pick: BusVertex, fraction: Option<u32>) -> bool {
    let Some(line) = lines
        .iter()
        .find(|l| l.feature_index == pick.feature && l.line_index == pick.line)
    else {
        return false;
    };
    let i = pick.vertex as usize;
    let Some(a) = line.coordinates.get(i) else {
        return false;
    };
    let q = i64::from(fraction.unwrap_or(0));
    if q > FRACTION_DENOMINATOR {
        return false;
    }
    let b = if q == 0 {
        a
    } else {
        let Some(b) = line.coordinates.get(i.saturating_add(1)) else {
            return false;
        };
        b
    };
    let bound = i64::from(line.extent) * FRACTION_DENOMINATOR;
    a.iter().zip(b).all(|(&a, &b)| {
        let p = i64::from(a) * (FRACTION_DENOMINATOR - q) + i64::from(b) * q;
        p > 0 && p < bound
    })
}
