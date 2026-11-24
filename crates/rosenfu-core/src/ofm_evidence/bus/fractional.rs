//! Transient native route search with interior boarding cuts. Not a saved
//! BusPathBinding or a network/cache mutation. Keep full raw atoms so routing,
//! physical resource IDs, and road traversal metrics agree with ordinary Bus.
use super::*;

/// The cuts belong to the first/last directed source atoms, in traversal
/// orientation. Neither cut changes the atom's raw ID or adds a graph node.
#[derive(Debug, Clone, PartialEq)]
pub struct FractionalBusTraversal {
    pub(super) path: BusPath,
    terminal_cuts: [u32; 2],
    operation_mm: u64,
}

impl FractionalBusTraversal {
    pub fn terminal_cuts(&self) -> [u32; 2] {
        self.terminal_cuts
    }

    pub fn length_mm(&self) -> u64 {
        self.operation_mm
    }

    pub fn atom_count(&self) -> usize {
        self.path.steps.len()
    }

    pub fn directed_atom_id(&self, index: usize) -> Option<String> {
        self.path.source_atom_id(index)
    }

    pub fn physical_atom_id(&self, index: usize) -> Option<String> {
        self.path.physical_atom_id(index)
    }

    pub fn oriented_route_points(&self) -> Vec<[f64; 2]> {
        let mut points = self.path.points.clone();
        let z = points.len() - 1;
        let at = |a: [f64; 2], b: [f64; 2], q: u32| {
            let t = f64::from(q) / f64::from(BUS_STOP_DENOMINATOR);
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        let start = at(points[0], points[1], self.terminal_cuts[0]);
        let end = at(points[z - 1], points[z], self.terminal_cuts[1]);
        points[0] = start;
        points[z] = end;
        points
    }
}

#[derive(Clone, Copy)]
struct Terminal {
    source: usize,
    arc: Arc,
    cut: u32,
}

fn terminal_directions(
    graph: &GameplayBusGraph,
    key: &str,
    vertex: BusVertex,
    q: u32,
    work: &mut usize,
) -> Result<Vec<Terminal>> {
    let Some(next) = vertex.vertex.checked_add(1) else {
        return Ok(vec![]);
    };
    let upper = BusVertex {
        vertex: next,
        ..vertex
    };
    let Some(lo) = graph.node_at(key, vertex) else {
        return Ok(vec![]);
    };
    let Some(hi) = graph.node_at(key, upper) else {
        return Ok(vec![]);
    };
    let fragment = format!(
        "ofm:{}:{}:{}",
        key.split_once("/planet/")
            .and_then(|(_, suffix)| suffix.strip_suffix(".pbf"))
            .ok_or(Error::Binding)?
            .replace('/', ":"),
        vertex.feature,
        vertex.line
    );
    let mut out = Vec::with_capacity(2);
    for (start, end, from, to, cut) in [
        (lo, hi, vertex.vertex, next, q),
        (hi, lo, next, vertex.vertex, BUS_STOP_DENOMINATOR - q),
    ] {
        let mut found = None;
        for &arc in &graph.adj[start] {
            *work = work.checked_sub(1).ok_or(Error::Limit)?;
            let line = &graph.lines[arc.line];
            if arc.to == end
                && line.tile_key == key
                && line.fragment_id == fragment
                && line.grade.is_ground()
                && line.boardable_ground
                && arc.ordinal == vertex.vertex as usize
                && arc.from_pos == f64::from(from)
                && arc.to_pos == f64::from(to)
            {
                if found.replace(arc).is_some() {
                    return Err(Error::Binding);
                }
            }
        }
        if let Some(arc) = found {
            out.push(Terminal {
                source: start,
                arc,
                cut,
            });
        }
    }
    Ok(out)
}

struct Chosen {
    source: usize,
    arcs: Vec<Arc>,
    cuts: [u32; 2],
    score: u128,
}
fn choose(picked: &mut Option<Chosen>, initial: Terminal, final_: Terminal, middle: Vec<Arc>) {
    let mut arcs = Vec::with_capacity(middle.len() + 2);
    arcs.push(initial.arc);
    arcs.extend(middle);
    arcs.push(final_.arc);
    let d = u128::from(BUS_STOP_DENOMINATOR);
    let score = arcs
        .iter()
        .map(|arc| u128::from(arc.weight) * d)
        .sum::<u128>()
        - u128::from(initial.arc.weight) * u128::from(initial.cut)
        - u128::from(final_.arc.weight) * (d - u128::from(final_.cut));
    if picked.as_ref().is_none_or(|before| score < before.score) {
        *picked = Some(Chosen {
            source: initial.source,
            arcs,
            cuts: [initial.cut, final_.cut],
            score,
        });
    }
}

fn terminal_metric_mm(path: &BusPath, cuts: [u32; 2]) -> Result<u64> {
    let units = &path.atom_operation_micrometres;
    let start = units.first().copied().ok_or(Error::Malformed)?;
    let end = units.last().copied().ok_or(Error::Malformed)?;
    let total = units.iter().map(|um| u128::from(*um)).sum::<u128>();
    let cut_um =
        |atom: u64, q: u32| u128::from(atom) * u128::from(q) / u128::from(BUS_STOP_DENOMINATOR);
    // Same global millimetre rounding as the native BusLeg metric, including
    // when the two cuts belong to a single atom.
    let from = (cut_um(start, cuts[0]) + 500) / 1000;
    let to = (total - u128::from(end) + cut_um(end, cuts[1]) + 500) / 1000;
    let length = to
        .checked_sub(from)
        .filter(|&mm| mm > 0)
        .ok_or(Error::Binding)?;
    u64::try_from(length).map_err(|_| Error::Limit)
}

/// The input binding scopes the same immutable Bus evidence rectangle as the
/// normal path solver. Here start/end refer to the LOWER raw vertex of each
/// selected road atom, and cuts are increasing raw order millionths. This
/// transient capability requires placed endpoints on complete ground atoms;
/// shared native continuation may traverse verified seams/interior structures.
/// Optional vias, loops, and return schedules are not accepted by this entry.
pub fn resolve_fractional_bus_traversal(
    store: &EvidenceStore,
    query: &BusPathBinding,
    cuts_raw: [u32; 2],
) -> Result<Option<FractionalBusTraversal>> {
    if query.has_fractional_stops()
        || query.pattern.is_some()
        || !query.interior_stops.is_empty()
        || !query.via.is_empty()
        || cuts_raw
            .iter()
            .any(|q| !(1..BUS_STOP_DENOMINATOR).contains(q))
    {
        return Err(Error::Binding);
    }
    let end_key = query.end_tile_key.as_deref().unwrap_or(&query.tile_key);
    if query.tile_key == end_key && query.start == query.end && cuts_raw[0] == cuts_raw[1] {
        return Err(Error::Binding);
    }
    let scope = store.scope(&query.tile_keys()?)?;
    for (key, digest) in std::iter::once((&query.tile_key, &query.tile_raw_sha256)).chain(
        query
            .additional_tiles
            .iter()
            .map(|p| (&p.tile_key, &p.tile_raw_sha256)),
    ) {
        if scope.decoded(key)?.binding.raw_sha256 != *digest {
            return Err(Error::Hash);
        }
    }
    let graph = build_gameplay_bus_graph(&scope, query)?;
    solve_fractional(&graph, query, cuts_raw)
}

pub(super) fn solve_fractional(
    graph: &GameplayBusGraph,
    query: &BusPathBinding,
    cuts_raw: [u32; 2],
) -> Result<Option<FractionalBusTraversal>> {
    let end_key = query.end_tile_key.as_deref().unwrap_or(&query.tile_key);
    let mut work = MAX_MULTI_RELAXATIONS;
    let starts = terminal_directions(graph, &query.tile_key, query.start, cuts_raw[0], &mut work)?;
    let ends = terminal_directions(graph, end_key, query.end, cuts_raw[1], &mut work)?;
    let mut chosen: Option<Chosen> = None;
    let mut scratch = SearchScratch::new(graph.geometry.len());
    for &start in &starts {
        for &end in &ends {
            if start.source == end.source
                && start.arc.to == end.arc.to
                && start.arc.line == end.arc.line
                && start.arc.ordinal == end.arc.ordinal
                && start.cut < end.cut
            {
                let score = u128::from(start.arc.weight) * u128::from(end.cut - start.cut);
                if chosen.as_ref().is_none_or(|before| score < before.score) {
                    chosen = Some(Chosen {
                        source: start.source,
                        arcs: vec![start.arc],
                        cuts: [start.cut, end.cut],
                        score,
                    });
                }
                continue;
            }
            let mut middle = Vec::new();
            if start.arc.to != end.source
                && !scratch.leg(graph, start.arc.to, end.source, &mut work, &mut middle)?
            {
                continue;
            }
            if middle.len() > MAX_MULTI_ATOMS - 2 {
                return Err(Error::Limit);
            }
            choose(&mut chosen, start, end, middle);
        }
    }
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let mut path = materialize_bus_path(graph, query, chosen.source, chosen.arcs, vec![])?;
    path.resolved_terminal_cuts = Some(chosen.cuts);
    let operation_mm = terminal_metric_mm(&path, chosen.cuts)?;
    Ok(Some(FractionalBusTraversal {
        path,
        terminal_cuts: chosen.cuts,
        operation_mm,
    }))
}
