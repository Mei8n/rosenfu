//! Small default tie preference only. Reservations and rear-clear stay Fixed.
use super::*;

const MAX_FIT_POINTS: usize = 256;
const MAX_PROOF_RUN_SECTIONS: usize = 64;
const EARTH_RADIUS_M: f64 = 6_371_000.0;
// Small chart at non-polar latitudes; allow one percent distance slack
// inside the parked envelope and fail closed outside these bounds.
const MAX_CHART_DEGREES: f64 = 0.05;
const MAX_MAIN_OFFSET_M: f64 = 0.25;
const MAX_BAY_OFFSET_M: f64 = 250.0;

struct OpposingBirthGeometry<'a> {
    alignments: BTreeMap<AlignmentId, &'a Alignment>,
    incident: BTreeMap<AlignmentId, Vec<usize>>,
    connections: &'a [AlignmentConnection],
    placements: BTreeMap<(StationId, AlignmentId), &'a StationPlacement>,
}

impl<'a> OpposingBirthGeometry<'a> {
    fn index(data: &'a WorldDraft) -> Self {
        let mut incident = BTreeMap::<_, Vec<_>>::new();
        for (i, link) in data.alignment_connections.iter().enumerate() {
            incident.entry(link.a.alignment_id).or_default().push(i);
            incident.entry(link.b.alignment_id).or_default().push(i);
        }
        let mut placements = BTreeMap::new();
        for p in &data.station_placements {
            placements
                .entry((p.station_id, p.alignment_id))
                .or_insert(p);
        }
        Self {
            alignments: data.alignments.iter().map(|a| (a.id, a)).collect(),
            incident,
            connections: &data.alignment_connections,
            placements,
        }
    }

    fn prove(&self, a: &ServiceRunSpec, b: &ServiceRunSpec) -> Option<(String, String)> {
        let (local, through) = match (a.legs.len(), b.legs.len()) {
            (2, 1) => (a, b),
            (1, 2) => (b, a),
            _ => return None,
        };
        // Geometry proof also protects a train already in the siding when
        // future timetable changes disqualify the next matched birth.
        if has_crossing_locks(local)
            || has_crossing_locks(through)
            || has_parallel_join_locks(local)
            || has_parallel_join_locks(through)
        {
            return None;
        }
        let [incoming, outbound] = local.legs.as_slice() else {
            return None;
        };
        let express = &through.legs[0];
        if incoming.from != express.to
            || outbound.to != express.from
            || incoming.to != outbound.from
        {
            return None;
        }

        let ids = |leg: &RunLeg| -> Option<Vec<AlignmentId>> {
            if leg.sections.len() > MAX_PROOF_RUN_SECTIONS {
                return None;
            }
            let mut order = Vec::new();
            for section in &leg.sections {
                let mut lanes = section.resources.iter().filter_map(|k| inline_lane_key(k));
                let (id, _, _) = lanes.next()?;
                if lanes.next().is_some() {
                    return None;
                }
                let id = AlignmentId(id);
                if order.last() != Some(&id) {
                    order.push(id);
                }
            }
            Some(order)
        };
        let [near, main, far]: [AlignmentId; 3] = ids(express)?.try_into().ok()?;
        let [start, bay]: [AlignmentId; 2] = ids(incoming)?.try_into().ok()?;
        let [same_bay, end]: [AlignmentId; 2] = ids(outbound)?.try_into().ok()?;
        if (start, same_bay, end) != (far, bay, near)
            || [near, main, far].contains(&bay)
            || [near, far].contains(&main)
            || near == far
        {
            return None;
        }

        let mut incident = BTreeSet::new();
        for id in [near, main, far, bay] {
            for &index in self.incident.get(&id)? {
                incident.insert(index);
                if incident.len() > 4 {
                    return None;
                }
            }
        }
        if incident.len() != 4 {
            return None;
        }
        let ends = |a, b| {
            incident.iter().find_map(|&i| {
                let link = &self.connections[i];
                match (link.a.alignment_id, link.b.alignment_id) {
                    pair if pair == (a, b) => Some((link.a.endpoint, link.b.endpoint)),
                    pair if pair == (b, a) => Some((link.b.endpoint, link.a.endpoint)),
                    _ => None,
                }
            })
        };
        let (near_port, main_before) = ends(near, main)?;
        let (main_after, far_port) = ends(main, far)?;
        let (near_sibling, bay_before) = ends(near, bay)?;
        let (bay_after, far_sibling) = ends(bay, far)?;
        if near_port != near_sibling
            || far_port != far_sibling
            || main_before == main_after
            || bay_before == bay_after
        {
            return None;
        }
        let get = |id| self.alignments.get(&id).copied();
        let (near, main, far, bay) = (get(near)?, get(main)?, get(far)?, get(bay)?);
        for a in [near, main, far, bay] {
            if a.owner != main.owner
                || !matches!(a.track_type, TrackType::Normal)
                || a.points.len() > MAX_FIT_POINTS
                || a.vertical_profile
                    .iter()
                    .chain(&a.section_profiles)
                    .any(|p| p.start_height_m != 0.0 || p.end_height_m != 0.0)
                || a.effective_infrastructure().iter().any(|s| {
                    s.level != InfrastructureLevel::Ground
                        || ([main.id, bay.id].contains(&a.id) && s.track_count != 1)
                })
            {
                return None;
            }
        }

        // Station occurrence is a physical stop on the siding, not a guessed
        // name/group. The held envelope must fit one native section before
        // either 30 m turnout lead. Must also prove non-overlap along the
        // entire parked interval, not just at its center.
        let station = StationId(incoming.to.parse().ok()?);
        let placement = self.placements.get(&(station, bay.id))?;
        let train = local.vehicle.length_m;
        if train > 80.0 || placement.length_m < train + 4.0 || placement.width_m < 4.1 {
            return None;
        }
        let length = bay.length_m();
        let platform_envelope = train.max(placement.length_m / 2.0) + 4.0;
        let turnout_clearance = platform_envelope + 35.0;
        if length * placement.position < turnout_clearance
            || length * (1.0 - placement.position) < turnout_clearance
            || incoming.sections.last()?.length_m < platform_envelope
            || outbound.sections.first()?.length_m < platform_envelope
        {
            return None;
        }
        let in_lane = incoming
            .sections
            .last()?
            .resources
            .iter()
            .find_map(|k| inline_lane_key(k))?;
        let out_lane = outbound
            .sections
            .first()?
            .resources
            .iter()
            .find_map(|k| inline_lane_key(k))?;
        if in_lane != out_lane || in_lane.0 != bay.id.0 {
            return None;
        }
        let limits = [
            placement.position - platform_envelope / length,
            placement.position + platform_envelope / length,
        ];
        prove_separated_hold(
            main,
            bay,
            [near, far],
            limits,
            placement.width_m / 2.0 + 4.0,
        )?;

        // Require the true opposing throat locks in both movements; the
        // through run must wait for inbound rear-clear, and the parked train
        // cannot move back toward it until its held outgoing throat clears.
        let holds = |leg: &RunLeg, key: &str| {
            leg.sections
                .iter()
                .flat_map(|s| &s.resources)
                .any(|r| r == key)
        };
        let near_lock = format!("rail:join:{}:{near_port:?}", near.id.0);
        let far_lock = format!("rail:join:{}:{far_port:?}", far.id.0);
        if !holds(incoming, &far_lock)
            || !holds(outbound, &near_lock)
            || !holds(express, &far_lock)
            || !holds(express, &near_lock)
            || holds(incoming, &near_lock)
            || holds(outbound, &far_lock)
        {
            return None;
        }
        Some((local.service_id.clone(), through.service_id.clone()))
    }
}

impl World {
    pub(super) fn opposing_birth_priorities(&self, op: &WorldOperation) -> Vec<(String, String)> {
        if op.invalidated || op.runtime.admission_model() != RailAdmissionModel::Fixed {
            return Vec::new();
        }
        let pairs = op.runtime.isolated_authority_pairs();
        if pairs.is_empty() {
            return Vec::new();
        }
        let geometry = OpposingBirthGeometry::index(&self.data);
        pairs
            .into_iter()
            .filter_map(|(a, b)| {
                geometry.prove(op.runtime.service_spec(a)?, op.runtime.service_spec(b)?)
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct ChordPoint {
    along: f64,
    across: f64,
}

/// The native connector already proves exact endpoint coordinates. This
/// extra proof checks separation while the local occupies the whole bay
/// envelope. Both tracks must be monotone in a small local tangent chart.
fn prove_separated_hold(
    main: &Alignment,
    bay: &Alignment,
    stems: [&Alignment; 2],
    parked_fractions: [f64; 2],
    minimum_gap_m: f64,
) -> Option<()> {
    prove_separated_hold_between(main, bay, stems, bay, parked_fractions, minimum_gap_m)
}

pub(super) fn prove_separated_hold_between(
    main: &Alignment,
    bay: &Alignment,
    stems: [&Alignment; 2],
    parked: &Alignment,
    parked_fractions: [f64; 2],
    minimum_gap_m: f64,
) -> Option<()> {
    let anchor = *main.points.first()?;
    if anchor.latitude.abs() > 60.0 || anchor.longitude.abs() > 179.0 {
        return None;
    }
    let lon_scale = anchor.latitude.to_radians().cos();
    let flat = |point: GeoPoint| {
        let dx = point.longitude - anchor.longitude;
        let dy = point.latitude - anchor.latitude;
        if dx.abs() > MAX_CHART_DEGREES || dy.abs() > MAX_CHART_DEGREES {
            return None;
        }
        Some((
            dx.to_radians() * EARTH_RADIUS_M * lon_scale,
            dy.to_radians() * EARTH_RADIUS_M,
        ))
    };
    let (dx, dy) = flat(*main.points.last()?)?;
    let span = dx.hypot(dy);
    if span <= 1.0 {
        return None;
    }
    let (east, north) = (dx / span, dy / span);
    let project = |p| {
        let (x, y) = flat(p)?;
        Some(ChordPoint {
            along: east * x + north * y,
            across: east * y - north * x,
        })
    };
    let trace = |line: &Alignment| -> Option<Vec<ChordPoint>> {
        let mut p = line
            .points
            .iter()
            .map(|p| project(*p))
            .collect::<Option<Vec<_>>>()?;
        if p.first()?.along > p.last()?.along {
            p.reverse();
        }
        p.windows(2)
            .all(|pair| pair[0].along + 1e-6 < pair[1].along)
            .then_some(p)
    };
    let nearly_straight = |path: &[ChordPoint]| {
        path.iter().all(|p| p.across.abs() <= MAX_MAIN_OFFSET_M)
            && path.windows(2).all(|pair| {
                (pair[1].across - pair[0].across).abs() <= (pair[1].along - pair[0].along) * 0.01
            })
    };
    let main = trace(main)?;
    let bypass = trace(bay)?;
    let bounds = [main.first()?.along, main.last()?.along];
    if [bypass.first()?.along, bypass.last()?.along] != bounds
        || !nearly_straight(&main)
        || bypass.iter().any(|p| p.across.abs() > MAX_BAY_OFFSET_M)
    {
        return None;
    }
    let mut entrance_side = 0usize;
    for stem in stems {
        let p = trace(stem)?;
        let ends = [p.first()?.along, p.last()?.along];
        if !nearly_straight(&p) {
            return None;
        }
        if ends[1] == bounds[0] {
            entrance_side += 1;
        } else if ends[0] != bounds[1] {
            return None;
        }
    }
    if entrance_side != 1 {
        return None;
    }

    let [a, b] = parked_fractions;
    let a = project(parked.point_at(a))?.along;
    let b = project(parked.point_at(b))?.along;
    let (left, right) = (a.min(b), a.max(b));
    if !(bounds[0] < left && left < right && right < bounds[1]) {
        return None;
    }
    let gap_at = |x| -> Option<f64> { Some(across_at(&bypass, x)? - across_at(&main, x)?) };
    let side = gap_at(left)?.signum();
    let gap_required = minimum_gap_m * 1.01 + 0.1;
    // A difference of two piecewise linear monotone traces is linear between
    // their combined vertices. Look at every breakpoint, and the clipped
    // endpoints of the actual parked interval, not only the station centre.
    for x in main
        .iter()
        .chain(&bypass)
        .map(|p| p.along)
        .chain([left, right])
    {
        if x == bounds[0] || x == bounds[1] {
            continue;
        }
        let gap = gap_at(x)? * side;
        if gap <= 1e-6 || ((left..=right).contains(&x) && gap < gap_required) {
            return None;
        }
    }
    Some(())
}

fn across_at(trace: &[ChordPoint], along: f64) -> Option<f64> {
    if along < trace.first()?.along || along > trace.last()?.along {
        return None;
    }
    let i = trace
        .partition_point(|p| p.along <= along)
        .min(trace.len() - 1);
    let (a, b) = (trace.get(i.checked_sub(1)?)?, trace.get(i)?);
    let f = (along - a.along) / (b.along - a.along);
    Some(a.across + (b.across - a.across) * f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_separation_proves_the_whole_envelope_with_reversed_construction() {
        let mut world = World::default();
        let user = world.add_player("Proof".into()).unwrap();
        let owner = world.add_company(user, "Rail".into()).unwrap();
        let (s, c) = 35_f64.to_radians().sin_cos();
        let point = |along: f64, across: f64| GeoPoint {
            latitude: 35.0 + ((along * s + across * c) / EARTH_RADIUS_M).to_degrees(),
            longitude: 139.0
                + ((along * c - across * s) / (EARTH_RADIUS_M * 35_f64.to_radians().cos()))
                    .to_degrees(),
        };
        let mut make = |name: &str, samples: &[(f64, f64)]| {
            world
                .build_alignment(
                    owner,
                    name.into(),
                    samples.iter().map(|&(x, y)| point(x, y)).collect(),
                )
                .unwrap()
        };
        let a = make("West reversed", &[(0.0, 0.0), (-500.0, 0.0)]);
        let b = make("East", &[(1_000.0, 0.0), (1_500.0, 0.0)]);
        let main = make("Main reversed", &[(1_000.0, 0.0), (0.0, 0.0)]);
        let bay = make(
            "Siding forward",
            &[(0.0, 0.0), (300.0, 55.0), (750.0, 55.0), (1_000.0, 0.0)],
        );
        let pinched = make(
            "Point inside the envelope pinches the spacing",
            &[(0.0, 0.0), (440.0, 5.0), (500.0, 50.0), (1_000.0, 0.0)],
        );
        let curved = make(
            "Unsupported main curve",
            &[(0.0, 0.0), (500.0, 0.4), (1_000.0, 0.0)],
        );
        let get = |id| {
            world
                .snapshot()
                .alignments
                .iter()
                .find(|a| a.id == id)
                .unwrap()
        };
        let check = |line, siding| {
            prove_separated_hold(get(line), get(siding), [get(a), get(b)], [0.40, 0.60], 16.0)
        };
        assert_eq!(check(main, bay), Some(()));
        assert_eq!(
            check(main, pinched),
            None,
            "center spacing alone is not proof"
        );
        assert_eq!(check(curved, bay), None);
    }
}
