//! Construction map model, not a claim of surveyed water completeness.
//! The bundled regional coarse model is an explicit fixed source, not a fallback
//! for failed network acquisition. Unsupported coordinates remain unavailable.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub const WATER_MODEL_REVISION: &str =
    "ed008c4333500d072a9b7084bca52f245424dfc33c60c0db499625b7d63c7bca";
const BYTES: &[u8] =
    include_bytes!("../../../../data/water-japan-coarse-20261114/japan-coarse.water");
pub(super) const UNKNOWN: &str = "水域モデルの情報が不足しているため着工できません。";
const GROUND: &str = "水域上に地上線路は敷設できません。";
const LONG: &str = "高架・開削の連続水面横断が30kmを超えます。";
const COMPLEX: &str = "水域の接続経路を検証しきれないため着工できません。";
#[derive(Clone)]
pub(super) struct Model {
    per: u32,
    rows: Vec<Vec<u32>>,
    scope: [f64; 4],
}
impl Model {
    fn decode(bytes: &[u8], scope: [f64; 4]) -> Result<Self, String> {
        if bytes.len() < 16 || bytes.len() > 16 * 1024 * 1024 || &bytes[..4] != b"TWW1" {
            return Err(UNKNOWN.into());
        }
        let read = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("checked header"));
        let (width, height, per) = (read(4), read(8), read(12));
        if per == 0 || per > 182 || width != 360 * per || height != 180 * per {
            return Err(UNKNOWN.into());
        }
        let mut offset = 16;
        let integer = |offset: &mut usize| -> Result<u32, String> {
            let mut value = 0u64;
            for shift in (0..=28).step_by(7) {
                let b = *bytes.get(*offset).ok_or(UNKNOWN)?;
                *offset += 1;
                value += u64::from(b & 127) << shift;
                if value > u64::from(u32::MAX) {
                    return Err(UNKNOWN.into());
                }
                if b & 128 == 0 {
                    if shift > 0 && b == 0 {
                        return Err(UNKNOWN.into());
                    }
                    return Ok(value as u32);
                }
            }
            Err(UNKNOWN.into())
        };
        let mut rows = Vec::with_capacity(height as usize);
        let mut edges = 0;
        for _ in 0..height {
            let count = integer(&mut offset)?;
            edges += count;
            if count % 2 != 0 || count > width + 1 || edges > 4_000_000 {
                return Err(UNKNOWN.into());
            }
            let mut row = Vec::with_capacity(count as usize);
            let mut end = 0u32;
            for i in 0..count {
                let delta = integer(&mut offset)?;
                if i > 0 && delta == 0 {
                    return Err(UNKNOWN.into());
                }
                end = end.checked_add(delta).ok_or(UNKNOWN)?;
                if end > width {
                    return Err(UNKNOWN.into());
                }
                row.push(end);
            }
            rows.push(row);
        }
        if offset != bytes.len() {
            return Err(UNKNOWN.into());
        }
        Ok(Self { per, rows, scope })
    }
    pub(super) fn water(&self, p: GeoPoint) -> Result<bool, String> {
        let [w, s, e, n] = self.scope;
        if !p.longitude.is_finite()
            || !p.latitude.is_finite()
            || p.longitude < w
            || p.longitude >= e
            || p.latitude < s
            || p.latitude >= n
        {
            return Err(UNKNOWN.into());
        }
        let row = ((p.latitude + 90.) * f64::from(self.per)).floor() as usize;
        let col = ((p.longitude + 180.) * f64::from(self.per)).floor() as u32;
        let edges = self.rows.get(row).ok_or(UNKNOWN)?;
        Ok(edges.partition_point(|v| *v <= col) % 2 == 1)
    }
    pub(super) fn cuts(&self, a: GeoPoint, b: GeoPoint) -> Result<Vec<f64>, String> {
        self.water(a)?;
        self.water(b)?;
        let mut result = vec![0., 1.];
        let per = f64::from(self.per);
        for (from, to) in [(a.longitude, b.longitude), (a.latitude, b.latitude)] {
            if from == to {
                continue;
            }
            let low = (from.min(to) * per).floor() as i64 + 1;
            let high = (from.max(to) * per).ceil() as i64;
            for edge in low..high {
                let t = (edge as f64 / per - from) / (to - from);
                if t > 0. && t < 1. {
                    result.push(t);
                }
            }
        }
        Ok(result)
    }
}
pub(super) fn bundled() -> Result<&'static Model, String> {
    static MODEL: OnceLock<Result<Model, String>> = OnceLock::new();
    MODEL
        .get_or_init(|| Model::decode(BYTES, [122., 20., 154., 46.]))
        .as_ref()
        .map_err(Clone::clone)
}
pub(super) trait WaterField {
    fn water(&self, p: GeoPoint) -> Result<bool, String>;
    fn cuts(&self, a: GeoPoint, b: GeoPoint) -> Result<Vec<f64>, String>;
}
impl WaterField for Model {
    fn water(&self, p: GeoPoint) -> Result<bool, String> {
        Model::water(self, p)
    }
    fn cuts(&self, a: GeoPoint, b: GeoPoint) -> Result<Vec<f64>, String> {
        Model::cuts(self, a, b)
    }
}
fn mix(a: GeoPoint, b: GeoPoint, t: f64) -> GeoPoint {
    GeoPoint {
        longitude: a.longitude + (b.longitude - a.longitude) * t,
        latitude: a.latitude + (b.latitude - a.latitude) * t,
    }
}
#[derive(Clone, Copy, Default)]
struct Run {
    prefix: f64,
    suffix: f64,
    whole: bool,
}
fn summarize(pieces: &[(bool, f64)]) -> Result<Run, String> {
    let mut current = 0.;
    for &(wet, length) in pieces {
        if wet {
            current += length;
        } else {
            current = 0.;
        }
        if current > 30000. + 1e-6 {
            return Err(LONG.into());
        }
    }
    Ok(Run {
        prefix: pieces.iter().take_while(|p| p.0).map(|p| p.1).sum(),
        suffix: pieces.iter().rev().take_while(|p| p.0).map(|p| p.1).sum(),
        whole: pieces.iter().all(|p| p.0),
    })
}
fn path_run(a: &Alignment, model: &impl WaterField, budget: &mut usize) -> Result<Run, String> {
    let anchors = a
        .anchor_indices
        .clone()
        .unwrap_or_else(|| vec![0, a.points.len() - 1]);
    let lengths = a.section_lengths_m();
    let total: f64 = lengths.iter().sum();
    let mut start = 0.;
    let mut pieces = vec![];
    for (index, range) in anchors.windows(2).enumerate() {
        let length = lengths[index];
        let profile = a
            .section_profiles
            .get(index)
            .copied()
            .or_else(|| {
                a.vertical_profile
                    .and_then(|p| p.clip(start / total, ((start + length) / total).min(1.)))
            })
            .unwrap_or(VerticalProfile {
                start_height_m: a
                    .section_infrastructure(index)
                    .ok_or(UNKNOWN)?
                    .level
                    .height_m(),
                end_height_m: a
                    .section_infrastructure(index)
                    .ok_or(UNKNOWN)?
                    .level
                    .height_m(),
                start_tangent_m: 0.,
                end_tangent_m: 0.,
            });
        let mut offset = 0.;
        for pair in a.points[range[0]..=range[1]].windows(2) {
            let distance = super::alignment::distance(pair[0], pair[1]);
            let p = profile
                .clip(
                    (offset / length).clamp(0., 1.),
                    ((offset + distance) / length).clamp(0., 1.),
                )
                .ok_or(UNKNOWN)?;
            // Hermite's cubic Bezier control hull proves the whole segment is
            // below -12m. Water availability cannot change either construction rule.
            if [
                p.start_height_m,
                p.start_height_m + p.start_tangent_m / 3.,
                p.end_height_m - p.end_tangent_m / 3.,
                p.end_height_m,
            ]
            .iter()
            .all(|h| *h <= -2. * LEVEL_SPACING_M + 1e-9)
            {
                pieces.push((false, distance));
                offset += distance;
                continue;
            }
            let mut cuts = model.cuts(pair[0], pair[1])?;
            let zeros = crate::water_observations::zero_candidates(p);
            for t in &zeros {
                if model.water(mix(pair[0], pair[1], *t))? {
                    return Err(GROUND.into());
                }
            }
            cuts.extend(zeros);
            let deep = VerticalProfile {
                start_height_m: p.start_height_m + 2. * LEVEL_SPACING_M,
                end_height_m: p.end_height_m + 2. * LEVEL_SPACING_M,
                ..p
            };
            cuts.extend(crate::water_observations::zero_candidates(deep));
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-13);
            *budget = budget.checked_sub(cuts.len()).ok_or(COMPLEX)?;
            for span in cuts.windows(2) {
                let mid = (span[0] + span[1]) / 2.;
                let wet = model.water(mix(pair[0], pair[1], mid))?;
                let h = p.height_at(mid);
                if wet && p.is_flat() && h.abs() <= 1e-9 {
                    return Err(GROUND.into());
                }
                pieces.push((
                    wet && h > -2. * LEVEL_SPACING_M + 1e-9,
                    distance * (span[1] - span[0]),
                ));
            }
            offset += distance;
        }
        start += length;
    }
    let run = summarize(&pieces)?;
    // A closed polyline's coordinate seam is part of the same physical alignment.
    if a.points.first() == a.points.last() && !run.whole && run.prefix + run.suffix > 30000. + 1e-6
    {
        return Err(LONG.into());
    }
    Ok(run)
}
/// Station construction follows the point at the physical platform centre,
/// not a footprint water percentage. Existing unchanged stations are grandfathered
/// when provider data changes; renaming never becomes a construction operation.
fn validate_station_centers(
    before: &WorldDraft,
    after: &WorldDraft,
    model: &impl WaterField,
) -> Result<(), String> {
    for station in &after.stations {
        let old = before.stations.iter().find(|s| s.id == station.id);
        let placement = after
            .station_placements
            .iter()
            .find(|p| p.station_id == station.id);
        let old_placement = before
            .station_placements
            .iter()
            .find(|p| p.station_id == station.id);
        let location = |world: &WorldDraft, placement: Option<&StationPlacement>| {
            placement.and_then(|p| {
                let a = world.alignments.iter().find(|a| a.id == p.alignment_id)?;
                Some((a.point_at(p.position), a.height_at_position_m(p.position)?))
            })
        };
        let current = location(after, placement);
        let previous = location(before, old_placement);
        let changed = old
            .is_none_or(|s| s.latitude != station.latitude || s.longitude != station.longitude)
            || placement.map(|p| (p.length_m, p.width_m))
                != old_placement.map(|p| (p.length_m, p.width_m))
            || current != previous;
        if !changed {
            continue;
        }
        let (point, height) = match (placement, current) {
            (Some(_), Some(value)) => value,
            (Some(_), None) => return Err(UNKNOWN.into()),
            (None, _) => (
                GeoPoint {
                    latitude: station.latitude,
                    longitude: station.longitude,
                },
                0.,
            ),
        };
        // A viaduct or underground platform has no ground-water restriction.
        // Preserve unavailable-data errors for ground platforms, never assume dry.
        if height.abs() <= 1e-9 && model.water(point)? {
            return Err("水域上に地上駅は設置できません。".into());
        }
    }
    Ok(())
}

pub(super) fn validate_with_model(
    before: &WorldDraft,
    after: &WorldDraft,
    model: &impl WaterField,
) -> Result<(), String> {
    after.validate().map_err(|e| e.to_string())?;
    validate_station_centers(before, after, model)?;
    // Match by stable identity, not iteration order. Keep the first match
    // just as the former linear lookup did, even for an external old draft.
    let mut previous_alignments = BTreeMap::new();
    for alignment in &before.alignments {
        previous_alignments.entry(alignment.id).or_insert(alignment);
    }
    let mut relevant: BTreeSet<_> = after
        .alignments
        .iter()
        .filter(|a| {
            previous_alignments.get(&a.id).is_none_or(|old| {
                old.points != a.points
                    || old.infrastructure != a.infrastructure
                    || old.vertical_profile != a.vertical_profile
                    || old.section_profiles != a.section_profiles
                    || old.anchor_indices != a.anchor_indices
            })
        })
        .map(|a| a.id)
        .collect();
    for c in &after.alignment_connections {
        if !before.alignment_connections.contains(c) {
            relevant.extend([c.a.alignment_id, c.b.alignment_id]);
        }
    }
    let mut pending: Vec<_> = relevant.iter().copied().collect();
    while let Some(id) = pending.pop() {
        for c in &after.alignment_connections {
            let next = if c.a.alignment_id == id {
                Some(c.b.alignment_id)
            } else if c.b.alignment_id == id {
                Some(c.a.alignment_id)
            } else {
                None
            };
            if let Some(next) = next {
                if relevant.insert(next) {
                    pending.push(next);
                }
            }
        }
    }
    let mut runs = BTreeMap::new();
    let mut budget = 500_000;
    for a in after.alignments.iter().filter(|a| relevant.contains(&a.id)) {
        let run = path_run(a, model, &mut budget)?;
        runs.insert((a.id, false), run);
        runs.insert(
            (a.id, true),
            Run {
                prefix: run.suffix,
                suffix: run.prefix,
                ..run
            },
        );
    }
    // Connections sharing a port form one junction node. Traversing a branch
    // must not require travelling down the stem before selecting another branch.
    let ports: BTreeMap<_, _> = runs
        .keys()
        .copied()
        .enumerate()
        .map(|(i, k)| (k, i))
        .collect();
    let mut parents: Vec<usize> = (0..ports.len()).collect();
    fn root(parents: &mut [usize], mut i: usize) -> usize {
        while parents[i] != i {
            parents[i] = parents[parents[i]];
            i = parents[i];
        }
        i
    }
    for c in &after.alignment_connections {
        let a = (c.a.alignment_id, c.a.endpoint == AlignmentEndpoint::End);
        let b = (c.b.alignment_id, c.b.endpoint == AlignmentEndpoint::End);
        if let (Some(&a), Some(&b)) = (ports.get(&a), ports.get(&b)) {
            let a = root(&mut parents, a);
            let b = root(&mut parents, b);
            parents[a] = b;
        }
    }
    let mut groups: BTreeMap<usize, Vec<(AlignmentId, bool)>> = BTreeMap::new();
    for (&port, &index) in &ports {
        let node = root(&mut parents, index);
        groups.entry(node).or_default().push(port);
    }
    let mut graph_budget = 100_000usize;
    for (&start, &first) in &runs {
        if first.suffix == 0. {
            continue;
        }
        let mut stack = vec![(start, first.suffix, BTreeSet::from([start.0]))];
        while let Some(((id, reverse), tail, used)) = stack.pop() {
            graph_budget = graph_budget.checked_sub(1).ok_or(COMPLEX)?;
            let exit = (id, !reverse);
            let node = root(&mut parents, *ports.get(&exit).ok_or(UNKNOWN)?);
            for &key in groups.get(&node).ok_or(UNKNOWN)? {
                if key == exit {
                    continue;
                }
                let run = runs.get(&key).ok_or(UNKNOWN)?;
                if used.contains(&key.0) {
                    if key == start && !first.whole && tail + first.prefix > 30000. + 1e-6 {
                        return Err(LONG.into());
                    }
                    continue;
                }
                if tail + run.prefix > 30000. + 1e-6 {
                    return Err(LONG.into());
                }
                if !run.whole || run.suffix == 0. {
                    continue;
                }
                if used.len() >= 4096 || stack.len() >= 4096 {
                    return Err(COMPLEX.into());
                }
                let mut visited = used.clone();
                visited.insert(key.0);
                stack.push((
                    key,
                    if run.whole {
                        tail + run.suffix
                    } else {
                        run.suffix
                    },
                    visited,
                ));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f64, y: f64) -> GeoPoint {
        GeoPoint {
            longitude: x,
            latitude: y,
        }
    }
    fn wet() -> Model {
        Model {
            per: 60,
            rows: vec![vec![0, 21600]; 10800],
            scope: [-180., -85., 180., 85.],
        }
    }
    fn fixture(points: Vec<GeoPoint>, height: f64) -> (WorldDraft, WorldDraft) {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        let owner = w.add_company(player, "c".into()).unwrap();
        let before = w.snapshot().clone();
        w.build_alignment(owner, "a".into(), points).unwrap();
        let mut after = w.snapshot().clone();
        after.alignments[0].vertical_profile = Some(VerticalProfile {
            start_height_m: height,
            end_height_m: height,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        });
        (before, after)
    }
    struct StationField {
        wet: bool,
        unavailable: bool,
    }
    impl WaterField for StationField {
        fn water(&self, _: GeoPoint) -> Result<bool, String> {
            if self.unavailable {
                Err("missing station water".into())
            } else {
                Ok(self.wet)
            }
        }
        fn cuts(&self, _: GeoPoint, _: GeoPoint) -> Result<Vec<f64>, String> {
            Ok(vec![0., 1.])
        }
    }
    fn station_fixture(height: f64) -> (WorldDraft, WorldDraft) {
        let (_, before) = fixture(vec![p(139.7, 35.6), p(139.72, 35.6)], height);
        let mut world = World::new(before.clone()).unwrap();
        let a = &before.alignments[0];
        world
            .place_station_on_alignment(a.owner, "s".into(), a.id, 0.5, 100., 10.)
            .unwrap();
        (before, world.snapshot().clone())
    }
    #[test]
    fn physical_water_delta_is_independent_of_alignment_order() {
        let (_, before) = fixture(vec![p(139.7, 35.6), p(139.72, 35.6)], 0.);
        let mut world = World::new(before).unwrap();
        let owner = world.snapshot().alignments[0].owner;
        world
            .build_alignment(owner, "b".into(), vec![p(139.7, 35.7), p(139.72, 35.7)])
            .unwrap();
        let before = world.snapshot().clone();
        let mut after = before.clone();
        after.alignments.reverse();
        after.alignments[0].name = "rename only".into();
        let wet = wet();
        validate_with_model(&before, &after, &wet).unwrap();
        after.alignments[0].points[0].longitude += 0.00001;
        assert_eq!(
            validate_with_model(&before, &after, &wet),
            Err(GROUND.into())
        );
    }
    #[test]
    fn station_center_rejects_ground_water_only_and_propagates_missing_data() {
        let wet = StationField {
            wet: true,
            unavailable: false,
        };
        let missing = StationField {
            wet: false,
            unavailable: true,
        };
        for height in [0., 6., -6., -12.] {
            let (before, after) = station_fixture(height);
            let result = validate_station_centers(&before, &after, &wet);
            if height == 0. {
                assert_eq!(result.unwrap_err(), "水域上に地上駅は設置できません。");
                assert_eq!(
                    validate_station_centers(&before, &after, &missing).unwrap_err(),
                    "missing station water"
                );
            } else {
                result.unwrap();
                validate_station_centers(&before, &after, &missing).unwrap();
            }
            validate_station_centers(
                &before,
                &after,
                &StationField {
                    wet: false,
                    unavailable: false,
                },
            )
            .unwrap();
        }
    }
    #[test]
    fn station_unchanged_rename_and_provider_refresh_do_not_reject_existing_station() {
        let (_, before) = station_fixture(0.);
        let mut after = before.clone();
        after.stations[0].name = "renamed".into();
        validate_with_model(
            &before,
            &after,
            &StationField {
                wet: true,
                unavailable: true,
            },
        )
        .unwrap();
    }
    #[test]
    fn station_reference_reparameterization_without_physical_change_is_not_new_construction() {
        let (_, before) = station_fixture(0.);
        let mut after = before.clone();
        let mut copy = after.alignments[0].clone();
        copy.id = AlignmentId(after.next_id);
        after.next_id += 1;
        after.station_placements[0].alignment_id = copy.id;
        after.alignments.push(copy);
        validate_station_centers(
            &before,
            &after,
            &StationField {
                wet: true,
                unavailable: true,
            },
        )
        .unwrap();
    }
    #[test]
    fn station_center_not_footprint_and_resize_rechecks_moved_center() {
        struct Shore;
        impl WaterField for Shore {
            fn water(&self, p: GeoPoint) -> Result<bool, String> {
                Ok(p.longitude > 139.71)
            }
            fn cuts(&self, _: GeoPoint, _: GeoPoint) -> Result<Vec<f64>, String> {
                Ok(vec![0., 1.])
            }
        }
        let (before, mut after) = station_fixture(0.);
        after.station_placements[0].position = 0.49;
        let point = after.alignments[0].point_at(0.49);
        after.stations[0].latitude = point.latitude;
        after.stations[0].longitude = point.longitude;
        after.station_placements[0].length_m = 1000.;
        after.station_placements[0].width_m = 100.;
        // Much of the platform crosses the shore, but its centre is on land.
        validate_station_centers(&before, &after, &Shore).unwrap();
        let before = after.clone();
        after.station_placements[0].position = 0.51;
        let point = after.alignments[0].point_at(0.51);
        after.stations[0].longitude = point.longitude;
        assert!(validate_station_centers(&before, &after, &Shore).is_err());
    }
    #[test]
    fn station_actual_height_change_is_rechecked_without_station_coordinate_change() {
        let (_, before) = station_fixture(6.);
        let mut after = before.clone();
        let profile = after.alignments[0].vertical_profile.as_mut().unwrap();
        profile.start_height_m = 0.;
        profile.end_height_m = 0.;
        assert!(validate_station_centers(
            &before,
            &after,
            &StationField {
                wet: true,
                unavailable: false
            }
        )
        .is_err());
    }
    #[test]
    fn bundled_examples_and_corruption() {
        let m = bundled().unwrap();
        assert!(!m.water(p(139.7671, 35.6812)).unwrap());
        assert!(m.water(p(136.1, 35.25)).unwrap());
        assert!(m.water(p(140., 33.)).unwrap());
        assert!(m.water(p(0., 0.)).is_err());
        assert!(Model::decode(&BYTES[..BYTES.len() - 1], [122., 20., 154., 46.]).is_err());
    }
    #[test]
    fn ground_and_slope_tangent_are_rejected_but_deep_tunnel_is_exempt() {
        let (before, mut after) = fixture(vec![p(0., 0.), p(1., 0.)], 0.);
        assert_eq!(
            validate_with_model(&before, &after, &wet()).unwrap_err(),
            GROUND
        );
        after.alignments[0].vertical_profile = Some(VerticalProfile {
            start_height_m: 6.,
            end_height_m: 6.,
            start_tangent_m: -24.,
            end_tangent_m: 24.,
        });
        assert_eq!(
            validate_with_model(&before, &after, &wet()).unwrap_err(),
            GROUND
        );
        for height in [-12., -18.] {
            after.alignments[0].vertical_profile = Some(VerticalProfile {
                start_height_m: height,
                end_height_m: height,
                start_tangent_m: 0.,
                end_tangent_m: 0.,
            });
            assert!(validate_with_model(&before, &after, &wet()).is_ok());
        }
        after.alignments[0].vertical_profile = Some(VerticalProfile {
            start_height_m: -6.,
            end_height_m: -6.,
            start_tangent_m: 0.,
            end_tangent_m: 0.,
        });
        assert_eq!(
            validate_with_model(&before, &after, &wet()).unwrap_err(),
            LONG
        );
    }
    #[test]
    fn thirty_km_is_allowed_and_positive_excess_rejected() {
        let degrees = 30000. / 6_371_000. * 180. / std::f64::consts::PI;
        let (before, mut after) = fixture(vec![p(0., 0.), p(degrees, 0.)], 6.);
        assert!(validate_with_model(&before, &after, &wet()).is_ok());
        after.alignments[0].points[1].longitude += 0.001 / 6_371_000. * 180. / std::f64::consts::PI;
        assert_eq!(
            validate_with_model(&before, &after, &wet()).unwrap_err(),
            LONG
        );
    }
    #[test]
    fn closed_seam_combines_wet_prefix_and_suffix_once() {
        let (before, after) = fixture(
            vec![p(0., 0.), p(0.3, 0.), p(0.3, 0.01), p(0., 0.01), p(0., 0.)],
            6.,
        );
        let mut model = wet();
        model.rows = vec![vec![0, 10812]; 10800]; // water west of 0.2 degrees
        assert_eq!(
            validate_with_model(&before, &after, &model).unwrap_err(),
            LONG
        );
        let (before, after) = fixture(
            vec![
                p(0., 0.),
                p(0.04, 0.),
                p(0.04, 0.04),
                p(0., 0.04),
                p(0., 0.),
            ],
            6.,
        );
        assert!(validate_with_model(&before, &after, &wet()).is_ok());
    }
    #[test]
    fn shared_junction_checks_branch_to_branch_not_only_stem_edges() {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        let owner = w.add_company(player, "c".into()).unwrap();
        let before = w.snapshot().clone();
        let stem = w
            .build_alignment(owner, "stem".into(), vec![p(-0.05, 0.), p(0., 0.)])
            .unwrap();
        let a = w
            .build_alignment(owner, "a".into(), vec![p(0., 0.), p(0.15, 0.)])
            .unwrap();
        let b = w
            .build_alignment(owner, "b".into(), vec![p(0., 0.), p(0.15, 0.001)])
            .unwrap();
        let mut after = w.snapshot().clone();
        for line in &mut after.alignments {
            line.infrastructure = vec![SectionInfrastructure {
                level: InfrastructureLevel::Elevated,
                track_count: 1,
            }];
        }
        for id in [a, b] {
            after.alignment_connections.push(AlignmentConnection {
                a: AlignmentEndpointRef {
                    alignment_id: stem,
                    endpoint: AlignmentEndpoint::End,
                },
                b: AlignmentEndpointRef {
                    alignment_id: id,
                    endpoint: AlignmentEndpoint::Start,
                },
            });
        }
        assert_eq!(
            validate_with_model(&before, &after, &wet()).unwrap_err(),
            LONG
        ); // each stem+branch is <30km
        after
            .alignments
            .iter_mut()
            .find(|line| line.id == b)
            .unwrap()
            .points[1] = p(0.1, 0.001);
        assert!(validate_with_model(&before, &after, &wet()).is_ok());
    }
    #[test]
    fn fragmented_deep_tunnel_keeps_distance_exemption() {
        let points = (0..=1000).map(|i| p(i as f64 / 1000., 0.)).collect();
        let (before, after) = fixture(points, -12.);
        assert!(validate_with_model(&before, &after, &wet()).is_ok());
    }
    #[test]
    fn long_dry_chain_does_not_consume_water_path_budget() {
        let mut w = World::default();
        let player = w.add_player("p".into()).unwrap();
        let owner = w.add_company(player, "c".into()).unwrap();
        let before = w.snapshot().clone();
        let mut ids = vec![];
        for i in 0..400 {
            ids.push(
                w.build_alignment(
                    owner,
                    "a".into(),
                    vec![p(i as f64 / 1000., 0.), p((i + 1) as f64 / 1000., 0.)],
                )
                .unwrap(),
            );
        }
        let mut after = w.snapshot().clone();
        for pair in ids.windows(2) {
            after.alignment_connections.push(AlignmentConnection {
                a: AlignmentEndpointRef {
                    alignment_id: pair[0],
                    endpoint: AlignmentEndpoint::End,
                },
                b: AlignmentEndpointRef {
                    alignment_id: pair[1],
                    endpoint: AlignmentEndpoint::Start,
                },
            });
        }
        let mut dry = wet();
        dry.rows = vec![vec![]; 10800];
        assert!(validate_with_model(&before, &after, &dry).is_ok());
    }
}
