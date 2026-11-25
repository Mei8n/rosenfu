//! Opt-in verification of an externally authenticated westbound road witness.
//! Not a saved route or a source of operating rights.
use super::{
    boundary_candidates, number, raw_road_fragment_id, raw_sha256, BusPath, Error, EvidenceStore,
    Result, TileBinding, TileLine,
};
use serde::{Deserialize, Serialize};

pub const WESTBOUND_SEAM_WITNESS_RULE: &str = "ofm_bus_westbound_shared_atom_z14_v1";
const EXTENT: i32 = 4096;
const MAX_RECEIPT_BYTES: usize = 4096;
type GridPoint = [i32; 2];

/// The independent attestor matches these immutable raw tiles to the producer
/// archive and verifies the named typed feature is the same original directed
/// way in both tiles. A tile URL or provider identifier alone is not attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WestboundSeamReceipt {
    pub rule: String,
    pub tiles_west_east: [TileBinding; 2],
    pub raw_feature_id: u64,
    pub vertices_west_east: [u32; 2],
}

/// Exact shared directed source segment, in units of 1/4096 tile. X is relative
/// to the boundary and Y to the tiles' common row. No connector is synthesized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestboundSeamWitness {
    pub fragments_west_east: [String; 2],
    pub vertices_west_east: [u32; 2],
    pub shared_atom: [GridPoint; 2],
}

/// Check receipt bytes against a digest obtained from an independent trusted
/// source. Do not pass a digest supplied alongside untrusted receipt bytes.
/// This checks exactly two adjacent immutable archive bindings, requires an
/// unambiguous appearance match in their complete context and verifies an
/// exact shared raw atom before the westbound boundary crossing. It neither
/// authenticates a producer by itself nor admits an operational path.
pub fn verify_westbound_seam_receipt(
    store: &EvidenceStore,
    receipt_bytes: &[u8],
    trusted_receipt_sha256: &str,
) -> Result<WestboundSeamWitness> {
    Ok(verify_receipt(store, receipt_bytes, trusted_receipt_sha256)?.0)
}

fn verify_receipt<'a>(
    store: &'a EvidenceStore,
    receipt_bytes: &[u8],
    trusted_receipt_sha256: &str,
) -> Result<(WestboundSeamWitness, WestboundSeamReceipt, &'a TileLine)> {
    if receipt_bytes.len() > MAX_RECEIPT_BYTES {
        return Err(Error::Limit);
    }
    if raw_sha256(receipt_bytes) != trusted_receipt_sha256 {
        return Err(Error::Hash);
    }
    let receipt: WestboundSeamReceipt =
        serde_json::from_slice(receipt_bytes).map_err(|_| Error::Malformed)?;
    if receipt.rule != WESTBOUND_SEAM_WITNESS_RULE {
        return Err(Error::Unsupported);
    }
    let [west, east] = &receipt.tiles_west_east;
    west.validate()?;
    east.validate()?;
    if west.y != east.y || west.x + 1 != east.x || west.revision != east.revision {
        return Err(Error::Binding);
    }
    let a = store.decoded(&west.canonical_url)?;
    let b = store.decoded(&east.canonical_url)?;
    if &a.binding != west || &b.binding != east {
        return Err(Error::Binding);
    }
    let a = a
        .unique_raw_road_line(receipt.raw_feature_id)
        .ok_or(Error::Binding)?;
    let b = b
        .unique_raw_road_line(receipt.raw_feature_id)
        .ok_or(Error::Binding)?;
    if a.extent != EXTENT as u32
        || b.extent != a.extent
        || a.attributes != b.attributes
        || a.attributes.get("oneway").and_then(number) != Some(1.)
        || ["access", "brunnel", "layer"]
            .iter()
            .any(|k| a.attributes.contains_key(*k))
    {
        return Err(Error::Binding);
    }
    let fragments = [raw_road_fragment_id(west, a), raw_road_fragment_id(east, b)];
    let candidates = boundary_candidates(store, &[(west.x, west.y), (east.x, east.y)])?;
    if !candidates.iter().any(|c| {
        c.a_fragment == fragments[0]
            && c.b_fragment == fragments[1]
            && c.max_separation_meters <= 0.05
    }) {
        return Err(Error::Binding);
    }

    let [a_index, b_index] = receipt.vertices_west_east.map(|i| i as usize);
    let (a_atom, (a_num, a_den)) = crossing_witness(&a.coordinates, a_index, EXTENT)?;
    let (b_atom, (b_num, b_den)) = crossing_witness(&b.coordinates, b_index, 0)?;
    // Compare exact rational cuts before any projection. Bound disagreement to
    // 1/10 grid quantum; only the *raw shared segment* is used as a witness.
    if a_atom != b_atom || 10 * (a_num * b_den - b_num * a_den).abs() > a_den * b_den {
        return Err(Error::Binding);
    }

    Ok((
        WestboundSeamWitness {
            fragments_west_east: fragments,
            vertices_west_east: receipt.vertices_west_east,
            shared_atom: a_atom,
        },
        receipt,
        a,
    ))
}

/// Exact occurrence of one complete raw source atom. Ordinal is its starting
/// vertex in the directed original (not a saved v1/v2 split edge ID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestboundSourceAtom {
    pub tile: TileBinding,
    pub fragment_id: String,
    pub raw_ordinal: u32,
}

/// Verified two-atom traversal through the seam, not an operational route.
/// Takes one east atom, hands off at its end, then takes one west atom. The
/// shared atom's unused alias is never traversed or charged twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WestboundSeamTraversal {
    points: [GridPoint; 3],
    atoms: [WestboundSourceAtom; 2],
}
impl WestboundSeamTraversal {
    /// X relative to the seam and Y relative to the common row, in 1/4096 tile units.
    pub fn grid_points(&self) -> &[GridPoint; 3] {
        &self.points
    }
    pub fn atoms_in_order(&self) -> &[WestboundSourceAtom; 2] {
        &self.atoms
    }

    /// Resource keys to reserve for the entire witnessed crossing. These are
    /// additional to edge keys, not replacements for any existing identity.
    pub fn fragment_resources(&self) -> [String; 2] {
        self.atoms.each_ref().map(fragment_resource)
    }

    /// Shared keys a legacy path would also have to reserve in the same
    /// trusted context. The checked model must use these on *both* sides of
    /// any new admission. Existing v1/v2 schedules stay untouched by default.
    /// Deliberately match the whole fragment, including old clipped aliases.
    pub fn legacy_fragment_resources(&self, route: &BusPath) -> Vec<String> {
        self.atoms
            .iter()
            .filter(|atom| self.legacy_uses_atom_fragment(route, atom))
            .map(fragment_resource)
            .collect()
    }

    /// A false result does not prove safe admission for mixed revisions or
    /// other road modes. Those require separate dispatch compatibility checks.
    pub fn overlaps_legacy_fragments(&self, route: &BusPath) -> bool {
        self.atoms
            .iter()
            .any(|atom| self.legacy_uses_atom_fragment(route, atom))
    }

    fn legacy_uses_atom_fragment(&self, route: &BusPath, atom: &WestboundSourceAtom) -> bool {
        route.spans.iter().any(|span| {
            span.binding_key == atom.tile.canonical_url && span.fragment_id == atom.fragment_id
        })
    }
}

fn fragment_resource(atom: &WestboundSourceAtom) -> String {
    // No timestamp/hash supplied by an operation save can rename this class.
    // Two clipped legs drawn from the same immutable source context must
    // contend even when their individual occurrence IDs or fractions differ.
    format!(
        "ofm-bus:fragment-lock:{}",
        raw_sha256(
            format!(
                "ofm-bus-fragment-lock-v1\0{}\0{}",
                atom.tile.canonical_url, atom.fragment_id
            )
            .as_bytes()
        )
    )
}

/// Reconstruct the minimal directed crossing from the immutable archive.
/// Both endpoints must be inside their respective tiles; no graph search,
/// synthetic geometry, route persistence, or authority to dispatch is implied.
pub fn replay_westbound_seam_traversal(
    store: &EvidenceStore,
    receipt_bytes: &[u8],
    trusted_receipt_sha256: &str,
) -> Result<WestboundSeamTraversal> {
    let (witness, receipt, west_line) =
        verify_receipt(store, receipt_bytes, trusted_receipt_sha256)?;
    let [west, east] = receipt.tiles_west_east;
    let [w, e] = witness.vertices_west_east;
    let [west_fragment, east_fragment] = witness.fragments_west_east;
    let [start, handoff] = witness.shared_atom;
    let [x, y] = west_line.coordinates[w as usize + 1];
    if !(0..EXTENT).contains(&start[0]) || !(0..EXTENT).contains(&x) {
        return Err(Error::Binding);
    }
    Ok(WestboundSeamTraversal {
        points: [start, handoff, [x - EXTENT, y]],
        atoms: [
            WestboundSourceAtom {
                tile: east,
                fragment_id: east_fragment,
                raw_ordinal: e - 1,
            },
            WestboundSourceAtom {
                tile: west,
                fragment_id: west_fragment,
                raw_ordinal: w,
            },
        ],
    })
}

fn crossing_witness(
    points: &[GridPoint],
    vertex: usize,
    offset: i32,
) -> Result<([GridPoint; 2], (i64, i64))> {
    let prior = points
        .get(vertex.checked_sub(1).ok_or(Error::Binding)?)
        .ok_or(Error::Binding)?;
    let at = points.get(vertex).ok_or(Error::Binding)?;
    let next = points
        .get(vertex.checked_add(1).ok_or(Error::Binding)?)
        .ok_or(Error::Binding)?;
    if !points.windows(2).all(|p| p[0][0] > p[1][0])
        || !(at[0] > offset && next[0] < offset)
        || points.iter().any(|p| !(0..EXTENT).contains(&p[1]))
    {
        return Err(Error::Binding);
    }
    // The decoder bounded all input coordinates to +/-16 extents. Products
    // below and their cross multiplication fit in i64.
    let d = i64::from(at[0] - next[0]);
    let n = i64::from(at[1]) * d + i64::from(next[1] - at[1]) * i64::from(at[0] - offset);
    Ok((
        [[prior[0] - offset, prior[1]], [at[0] - offset, at[1]]],
        (n, d),
    ))
}
