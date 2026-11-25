use super::*;

const MAX_LINKS: usize = 10_000;
const MAX_DIRECT_METRES: f64 = 1_000.0;

/// A player-authored directed station passage. Does not combine station identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationWalkLink {
    pub from: StationId,
    pub to: StationId,
    pub seconds: u16,
}

impl WorldDraft {
    pub(super) fn validate_station_walk_links(&self) -> Result<(), WorldError> {
        if self.station_walk_links.is_empty() {
            return Ok(());
        }
        if self.station_walk_links.len() > MAX_LINKS {
            return Err(WorldError::InvalidStationWalkLink);
        }
        let stations: BTreeMap<_, _> = self.stations.iter().map(|s| (s.id, s)).collect();
        let groups: BTreeMap<_, _> = self
            .station_groups
            .iter()
            .flat_map(|group| {
                group
                    .members
                    .iter()
                    .map(|member| (*member, group.station_id))
            })
            .collect();
        let identity = |id| groups.get(&id).copied().unwrap_or(id);
        let mut seen = BTreeSet::new();
        for link in &self.station_walk_links {
            let from = stations
                .get(&link.from)
                .ok_or(WorldError::InvalidStationWalkLink)?;
            let to = stations
                .get(&link.to)
                .ok_or(WorldError::InvalidStationWalkLink)?;
            let point = |s: &Station| GeoPoint {
                latitude: s.latitude,
                longitude: s.longitude,
            };
            let dist = alignment::distance(point(from), point(to));
            if identity(link.from) == identity(link.to)
                || from.owner != to.owner
                || !dist.is_finite()
                || dist > MAX_DIRECT_METRES
                || !(60..=3600).contains(&link.seconds)
                || !seen.insert((link.from, link.to))
            {
                return Err(WorldError::InvalidStationWalkLink);
            }
        }
        Ok(())
    }

    /// Use only explicitly saved passages for the fixed-OD solver. A nearby
    /// station, shared name, or station group does not establish a walk link.
    pub fn fixed_od_station_walks(&self) -> impl Iterator<Item = crate::demand::WalkLink> + '_ {
        self.station_walk_links
            .iter()
            .map(|link| crate::demand::WalkLink {
                from: link.from.0.to_string(),
                to: link.to.0.to_string(),
                seconds: f64::from(link.seconds),
            })
    }
}
