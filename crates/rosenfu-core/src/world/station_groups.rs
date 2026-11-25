//! A station identity shared by multiple physical platforms. Platform IDs remain
//! stable for saved routes/services; grouping does not invent track connections.
use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationGroup {
    pub station_id: StationId,
    pub members: Vec<StationId>,
}
impl WorldDraft {
    pub fn station_group_id(&self, id: StationId) -> StationId {
        self.station_groups
            .iter()
            .find(|g| g.members.contains(&id))
            .map_or(id, |g| g.station_id)
    }
    pub fn rename_station_group(&mut self, id: StationId, name: String) -> Result<(), WorldError> {
        if !self.stations.iter().any(|s| s.id == id) {
            return Err(WorldError::MissingReference("station"));
        }
        let members = self
            .station_groups
            .iter()
            .find(|g| g.members.contains(&id))
            .map(|g| g.members.clone())
            .unwrap_or_else(|| vec![id]);
        for station in &mut self.stations {
            if members.contains(&station.id) {
                station.name = name.clone();
            }
        }
        Ok(())
    }
    /// Called after removing a physical platform. Never removes other platforms.
    pub fn remove_station_group_member(&mut self, id: StationId) {
        for group in &mut self.station_groups {
            group.members.retain(|member| *member != id);
            if let Some(first) = group.members.first() {
                group.station_id = *first;
            }
        }
        self.station_groups.retain(|group| group.members.len() >= 2);
        self.station_groups.sort_by_key(|group| group.station_id);
    }
    pub(super) fn validate_station_groups(&self) -> Result<(), WorldError> {
        if self.station_groups.is_empty() {
            return Ok(());
        }
        let stations: BTreeMap<_, _> = self.stations.iter().map(|s| (s.id, s)).collect();
        let placed: BTreeSet<_> = self
            .station_placements
            .iter()
            .map(|p| p.station_id)
            .collect();
        let mut seen = BTreeSet::new();
        for group in &self.station_groups {
            if group.members.len() < 2
                || group.members.first() != Some(&group.station_id)
                || group.members.windows(2).any(|w| w[0] >= w[1])
            {
                return Err(WorldError::InvalidStationGroup);
            }
            let representative = stations
                .get(&group.station_id)
                .ok_or(WorldError::InvalidStationGroup)?;
            for id in &group.members {
                let station = stations.get(id).ok_or(WorldError::InvalidStationGroup)?;
                if !seen.insert(*id)
                    || station.owner != representative.owner
                    || station.name != representative.name
                    || !placed.contains(id)
                {
                    return Err(WorldError::InvalidStationGroup);
                }
            }
        }
        Ok(())
    }
    pub(super) fn join_station_group(&mut self, id: StationId, intersecting: &[StationId]) {
        if intersecting.is_empty() {
            return;
        }
        let mut members: BTreeSet<_> = intersecting.iter().copied().chain([id]).collect();
        for group in &self.station_groups {
            if group.members.iter().any(|m| intersecting.contains(m)) {
                members.extend(group.members.iter().copied());
            }
        }
        self.station_groups
            .retain(|g| !g.members.iter().any(|m| members.contains(m)));
        let members: Vec<_> = members.into_iter().collect();
        let representative = members[0];
        // Older existing station names win deterministically when groups merge.
        if let Some(name) = self
            .stations
            .iter()
            .find(|s| s.id == representative)
            .map(|s| s.name.clone())
        {
            for station in &mut self.stations {
                if members.contains(&station.id) {
                    station.name = name.clone();
                }
            }
        }
        self.station_groups.push(StationGroup {
            station_id: representative,
            members,
        });
        self.station_groups.sort_by_key(|g| g.station_id);
    }
}

pub(super) fn footprint(
    alignment: &Alignment,
    position: f64,
    length: f64,
    width: f64,
) -> Vec<GeoPoint> {
    let total = alignment.length_m();
    let half = length / total / 2.0;
    let start = position - half;
    let end = position + half;
    let mut spine = vec![alignment.point_at(start)];
    let mut offset = 0.0;
    for pair in alignment.points.windows(2) {
        let dx = (pair[1].longitude - pair[0].longitude).to_radians()
            * ((pair[0].latitude + pair[1].latitude) / 2.)
                .to_radians()
                .cos();
        let dy = (pair[1].latitude - pair[0].latitude).to_radians();
        offset += 6_371_000. * dx.hypot(dy);
        if offset / total > start && offset / total < end {
            spine.push(pair[1]);
        }
    }
    spine.push(alignment.point_at(end));
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (i, p) in spine.iter().enumerate() {
        let a = spine[i.saturating_sub(1)];
        let b = spine[(i + 1).min(spine.len() - 1)];
        let cos = p.latitude.to_radians().cos();
        let dx = (b.longitude - a.longitude) * cos;
        let dy = b.latitude - a.latitude;
        let norm = dx.hypot(dy);
        let norm = if norm == 0. { 1. } else { norm };
        let lat = (dx / norm * width / 2. / 6_371_000.).to_degrees();
        let lon = (-dy / norm * width / 2. / 6_371_000. / cos).to_degrees();
        left.push(GeoPoint {
            latitude: p.latitude + lat,
            longitude: p.longitude + lon,
        });
        right.push(GeoPoint {
            latitude: p.latitude - lat,
            longitude: p.longitude - lon,
        });
    }
    left.extend(right.into_iter().rev());
    left
}
pub(super) fn overlaps(a: &[GeoPoint], b: &[GeoPoint]) -> bool {
    fn contains(polygon: &[GeoPoint], p: GeoPoint) -> bool {
        let mut inside = false;
        for i in 0..polygon.len() {
            let a = polygon[i];
            let b = polygon[(i + 1) % polygon.len()];
            if (a.latitude > p.latitude) != (b.latitude > p.latitude)
                && p.longitude
                    < (b.longitude - a.longitude) * (p.latitude - a.latitude)
                        / (b.latitude - a.latitude)
                        + a.longitude
            {
                inside = !inside;
            }
        }
        inside
    }
    fn side(p: GeoPoint, q: GeoPoint, r: GeoPoint) -> f64 {
        (q.longitude - p.longitude) * (r.latitude - p.latitude)
            - (q.latitude - p.latitude) * (r.longitude - p.longitude)
    }
    if a.iter().any(|p| contains(b, *p)) || b.iter().any(|p| contains(a, *p)) {
        return true;
    }
    a.iter().enumerate().any(|(i, p)| {
        b.iter().enumerate().any(|(j, r)| {
            let q = a[(i + 1) % a.len()];
            let s = b[(j + 1) % b.len()];
            side(*p, q, *r) * side(*p, q, s) < 0. && side(*r, s, *p) * side(*r, s, q) < 0.
        })
    })
}
