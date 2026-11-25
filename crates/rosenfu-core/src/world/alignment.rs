use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}
/// Infrastructure class; vehicle performance is not yet simulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackType {
    Normal,
    HighSpeed,
    Tram,
}
impl TrackType {
    /// Provisional gameplay defaults, not real-world infrastructure standards.
    pub fn default_design_speed_kmh(self) -> f64 {
        match self {
            Self::Normal => 120.0,
            Self::HighSpeed => 300.0,
            Self::Tram => 50.0,
        }
    }
}
/// Categorical construction level, not metres of elevation or a gradient profile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfrastructureLevel {
    #[default]
    Ground,
    Elevated,
    #[serde(rename = "elevated_2")]
    Elevated2,
    #[serde(rename = "elevated_3")]
    Elevated3,
    Underground,
    #[serde(rename = "underground_2")]
    Underground2,
    #[serde(rename = "underground_3")]
    Underground3,
}
/// Physical infrastructure attributes of one anchor-to-anchor section.
/// Track count is not a simulated capacity, directional assignment, or lane topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionInfrastructure {
    pub track_count: u8,
    pub level: InfrastructureLevel,
}
impl Default for SectionInfrastructure {
    fn default() -> Self {
        Self {
            track_count: 1,
            level: InfrastructureLevel::Ground,
        }
    }
}
impl SectionInfrastructure {
    pub fn is_valid(self) -> bool {
        matches!(self.track_count, 1 | 2 | 4)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alignment {
    pub id: AlignmentId,
    pub owner: CompanyId,
    pub name: String,
    pub points: Vec<GeoPoint>,
    /// Original construction anchor boundaries in the sampled line. Legacy: one section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_indices: Option<Vec<usize>>,
    /// Empty is the compact encoding of single-track ground infrastructure throughout.
    /// Otherwise exactly one item per anchor interval, in geometry order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub infrastructure: Vec<SectionInfrastructure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_profile: Option<VerticalProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section_profiles: Vec<VerticalProfile>,
    pub track_type: TrackType,
    /// None preserves legacy saves and resolves to the track-type default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_speed_kmh: Option<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationPlacement {
    pub station_id: StationId,
    pub alignment_id: AlignmentId,
    pub position: f64,
    pub length_m: f64,
    pub width_m: f64,
}

pub(super) fn distance(a: GeoPoint, b: GeoPoint) -> f64 {
    let x = (b.longitude - a.longitude).to_radians()
        * ((a.latitude + b.latitude) / 2.0).to_radians().cos();
    let y = (b.latitude - a.latitude).to_radians();
    6_371_000.0 * x.hypot(y)
}
impl Alignment {
    pub fn section_count(&self) -> usize {
        self.anchor_indices
            .as_ref()
            .map_or(1, |indices| indices.len().saturating_sub(1))
    }
    pub fn section_infrastructure(&self, index: usize) -> Option<SectionInfrastructure> {
        (index < self.section_count())
            .then(|| self.infrastructure.get(index).copied().unwrap_or_default())
    }
    pub fn effective_infrastructure(&self) -> Vec<SectionInfrastructure> {
        if self.infrastructure.is_empty() {
            vec![SectionInfrastructure::default(); self.section_count()]
        } else {
            self.infrastructure.clone()
        }
    }

    pub fn effective_design_speed_kmh(&self) -> f64 {
        self.design_speed_kmh
            .unwrap_or_else(|| self.track_type.default_design_speed_kmh())
    }
    pub fn length_m(&self) -> f64 {
        self.points.windows(2).map(|p| distance(p[0], p[1])).sum()
    }
    pub fn point_at(&self, position: f64) -> GeoPoint {
        let mut remaining = self.length_m() * position;
        for pair in self.points.windows(2) {
            let length = distance(pair[0], pair[1]);
            if remaining <= length && length > 0.0 {
                let t = remaining / length;
                return GeoPoint {
                    latitude: pair[0].latitude + (pair[1].latitude - pair[0].latitude) * t,
                    longitude: pair[0].longitude + (pair[1].longitude - pair[0].longitude) * t,
                };
            }
            remaining -= length;
        }
        *self.points.last().expect("validated alignment")
    }
}
impl WorldDraft {
    pub(super) fn validate_alignments(&self) -> Result<(), WorldError> {
        for a in &self.alignments {
            name(&a.name, "alignment")?;
            if a.anchor_indices.as_ref().is_some_and(|indices| {
                indices.len() < 2
                    || indices.first() != Some(&0)
                    || indices.last().copied() != a.points.len().checked_sub(1)
                    || indices.windows(2).any(|pair| pair[0] >= pair[1])
            }) {
                return Err(WorldError::InvalidAlignment);
            }
            if (!a.infrastructure.is_empty() && a.infrastructure.len() != a.section_count())
                || a.infrastructure.iter().any(|item| !item.is_valid())
            {
                return Err(WorldError::InvalidAlignment);
            }
            if a.vertical_profile.is_none()
                && a.section_profiles.is_empty()
                && a.infrastructure
                    .windows(2)
                    .any(|sections| sections[0].level != sections[1].level)
            {
                // A category change is not a vertical ramp: reject instantaneous steps.
                return Err(WorldError::InvalidAlignment);
            }
            if a.vertical_profile
                .is_some_and(|profile| !profile.is_valid(a.length_m(), a.track_type))
            {
                return Err(WorldError::InvalidAlignment);
            }
            if !a.section_profiles.is_empty() {
                let lengths = a.section_lengths_m();
                if a.vertical_profile.is_some()
                    || a.section_profiles.len() != a.section_count()
                    || a.section_profiles
                        .iter()
                        .zip(&lengths)
                        .any(|(p, length)| !p.is_valid(*length, a.track_type))
                {
                    return Err(WorldError::InvalidAlignment);
                }
                for i in 1..a.section_profiles.len() {
                    if (a.section_profiles[i - 1].end_height_m
                        - a.section_profiles[i].start_height_m)
                        .abs()
                        > 1e-6
                        || (a.section_profiles[i - 1].end_tangent_m / lengths[i - 1]
                            - a.section_profiles[i].start_tangent_m / lengths[i])
                            .abs()
                            > 1e-8
                    {
                        return Err(WorldError::InvalidAlignment);
                    }
                }
            }
            let speed = a.effective_design_speed_kmh();
            if !speed.is_finite() || !(1.0..=600.0).contains(&speed) {
                return Err(WorldError::InvalidAlignmentDesignSpeed);
            }
            if !self.companies.iter().any(|c| c.id == a.owner) {
                return Err(WorldError::MissingReference("alignment.owner"));
            }
            if !(2..=4096).contains(&a.points.len())
                || a.points.iter().any(|p| {
                    !p.latitude.is_finite()
                        || !p.longitude.is_finite()
                        || !(-85.0..=85.0).contains(&p.latitude)
                        || !(-180.0..=180.0).contains(&p.longitude)
                })
                || a.points.windows(2).any(|p| distance(p[0], p[1]) < 0.01)
                || a.length_m() < 20.0
            {
                return Err(WorldError::InvalidAlignment);
            }
        }
        if self.station_placements.is_empty() {
            return Ok(());
        }
        // WorldDraft::validate has already rejected duplicate alignment and
        // station IDs. Resolve placements once by ID, not by rescanning the
        // whole map for every platform. Keep validation in stored order.
        let alignments: BTreeMap<_, _> = self.alignments.iter().map(|a| (a.id, a)).collect();
        let stations: BTreeMap<_, _> = self.stations.iter().map(|s| (s.id, s)).collect();
        let mut placed = BTreeSet::new();
        for p in &self.station_placements {
            let a = alignments
                .get(&p.alignment_id)
                .ok_or(WorldError::MissingReference("placement.alignment"))?;
            let s = stations
                .get(&p.station_id)
                .ok_or(WorldError::MissingReference("placement.station"))?;
            let length = a.length_m();
            let footprint_start = p.position - p.length_m / (2.0 * length);
            let footprint_end = p.position + p.length_m / (2.0 * length);
            let flat = a.is_flat_over_position(footprint_start, footprint_end);
            if !flat
                && footprint_start.is_finite()
                && footprint_end.is_finite()
                && footprint_start >= -1e-9
                && footprint_end <= 1. + 1e-9
                && footprint_start <= footprint_end
            {
                return Err(WorldError::StationOnGradient);
            }
            if !flat
                || !placed.insert(p.station_id)
                || a.owner != s.owner
                || !p.position.is_finite()
                || !(0.0..=1.0).contains(&p.position)
                || !p.length_m.is_finite()
                || !(20.0..=1000.0).contains(&p.length_m)
                || !p.width_m.is_finite()
                || !(4.0..=100.0).contains(&p.width_m)
                || p.length_m > length + 0.001
            {
                return Err(WorldError::InvalidStationPlacement);
            }
            let half = p.length_m / length / 2.0;
            if p.position < half - 1e-9 || p.position > 1.0 - half + 1e-9 {
                return Err(WorldError::InvalidStationPlacement);
            }
            let point = a.point_at(p.position);
            if distance(
                point,
                GeoPoint {
                    latitude: s.latitude,
                    longitude: s.longitude,
                },
            ) > 0.1
            {
                return Err(WorldError::InvalidStationPlacement);
            }
        }
        Ok(())
    }
}
/// Endpoint to extend. Input points run from the existing endpoint outwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentEndpoint {
    Start,
    End,
}

impl World {
    /// Extend a free-drawn alignment without moving any existing station.
    /// Service traversals must remain identical; source-provenance protection remains active.
    pub fn extend_alignment(
        &mut self,
        owner: CompanyId,
        alignment_id: AlignmentId,
        endpoint: AlignmentEndpoint,
        points: Vec<GeoPoint>,
    ) -> Result<(), WorldError> {
        let end = AlignmentEndpointRef {
            alignment_id,
            endpoint,
        };
        if self
            .data
            .alignment_connections
            .iter()
            .any(|link| link.a == end || link.b == end)
        {
            return Err(WorldError::InvalidAlignmentConnection);
        }
        let mut data = self.snapshot().clone();
        let alignment = data
            .alignments
            .iter_mut()
            .find(|a| a.id == alignment_id)
            .ok_or(WorldError::MissingReference("alignment"))?;
        let old_endpoint = match endpoint {
            AlignmentEndpoint::Start => alignment.points[0],
            AlignmentEndpoint::End => *alignment
                .points
                .last()
                .ok_or(WorldError::InvalidAlignment)?,
        };
        if (alignment.vertical_profile.is_some() || !alignment.section_profiles.is_empty())
            || alignment.owner != owner
            || points.len() < 2
            || points.first() != Some(&old_endpoint)
        {
            return Err(WorldError::InvalidAlignment);
        }
        // Sampled tangent arcs have at most 2.5 degrees of chord deviation.
        // Reject backward or sharp joins rather than silently constructing a cusp.
        let interior = match endpoint {
            AlignmentEndpoint::Start => alignment.points[1],
            AlignmentEndpoint::End => alignment.points[alignment.points.len() - 2],
        };
        let vector = |from: GeoPoint, to: GeoPoint| {
            (
                (to.longitude - from.longitude) * old_endpoint.latitude.to_radians().cos(),
                to.latitude - from.latitude,
            )
        };
        let incoming = vector(interior, old_endpoint);
        let outgoing = vector(old_endpoint, points[1]);
        let norm = incoming.0.hypot(incoming.1) * outgoing.0.hypot(outgoing.1);
        if !norm.is_finite()
            || norm <= 0.0
            || (incoming.0 * outgoing.0 + incoming.1 * outgoing.1) / norm < 5_f64.to_radians().cos()
        {
            return Err(WorldError::InvalidAlignment);
        }
        let old_length = alignment.length_m();
        let added_length: f64 = points.windows(2).map(|p| distance(p[0], p[1])).sum();
        if !added_length.is_finite() || added_length < 20.0 {
            return Err(WorldError::InvalidAlignment);
        }
        let prepend = if endpoint == AlignmentEndpoint::Start {
            points.len() - 1
        } else {
            0
        };
        let old_boundaries = alignment
            .anchor_indices
            .clone()
            .unwrap_or_else(|| vec![0, alignment.points.len() - 1]);
        if !alignment.infrastructure.is_empty() {
            let inherited = match endpoint {
                AlignmentEndpoint::Start => alignment.infrastructure[0],
                AlignmentEndpoint::End => *alignment
                    .infrastructure
                    .last()
                    .ok_or(WorldError::InvalidAlignment)?,
            };
            match endpoint {
                AlignmentEndpoint::Start => alignment.infrastructure.insert(0, inherited),
                AlignmentEndpoint::End => alignment.infrastructure.push(inherited),
            }
        }
        let old_point_count = alignment.points.len();
        let added_point_count = points.len() - 1;
        match endpoint {
            AlignmentEndpoint::Start => {
                let mut extended: Vec<_> = points.into_iter().skip(1).rev().collect();
                extended.extend_from_slice(&alignment.points);
                alignment.points = extended;
            }
            AlignmentEndpoint::End => alignment.points.extend(points.into_iter().skip(1)),
        }
        alignment.anchor_indices = Some(match endpoint {
            AlignmentEndpoint::Start => std::iter::once(0)
                .chain(old_boundaries.into_iter().map(|i| i + added_point_count))
                .collect(),
            AlignmentEndpoint::End => old_boundaries
                .into_iter()
                .chain(std::iter::once(old_point_count + added_point_count - 1))
                .collect(),
        });
        let new_length = alignment.length_m();
        let offset = if endpoint == AlignmentEndpoint::Start {
            added_length
        } else {
            0.0
        };
        for placement in &mut data.station_placements {
            if placement.alignment_id == alignment_id {
                placement.position = (offset + placement.position * old_length) / new_length;
            }
        }
        // This command alone constructs an append/prepend-only candidate: no old
        // geometry, station coordinates, routes, service legs or IDs are editable.
        // Generic replace deliberately retains its stronger edit prohibition.
        for p in data
            .alignment_provenance
            .iter_mut()
            .filter(|p| p.alignment_id == alignment_id)
        {
            p.start_point_index = p
                .start_point_index
                .checked_add(prepend)
                .ok_or(WorldError::InvalidProvenance)?;
            p.end_point_index = p
                .end_point_index
                .checked_add(prepend)
                .ok_or(WorldError::InvalidProvenance)?;
        }
        for p in data
            .road_path_provenance
            .iter_mut()
            .filter(|p| p.alignment_id == alignment_id)
        {
            p.start_point_index = p
                .start_point_index
                .checked_add(prepend)
                .ok_or(WorldError::InvalidProvenance)?;
            p.end_point_index = p
                .end_point_index
                .checked_add(prepend)
                .ok_or(WorldError::InvalidProvenance)?;
        }
        data.validate()?;
        data.preserve_extended_provenance(&self.data, alignment_id, prepend)?;
        data.preserve_service_traversals(&self.data)?;
        self.data = data.into();
        Ok(())
    }
}

impl World {
    pub fn build_alignment(
        &mut self,
        owner: CompanyId,
        name: String,
        points: Vec<GeoPoint>,
    ) -> Result<AlignmentId, WorldError> {
        self.build_alignment_typed(owner, name, points, TrackType::Normal)
    }
    pub fn build_alignment_typed(
        &mut self,
        owner: CompanyId,
        name: String,
        points: Vec<GeoPoint>,
        track_type: TrackType,
    ) -> Result<AlignmentId, WorldError> {
        self.insert(|data, id| {
            data.alignments.push(Alignment {
                id: AlignmentId(id),
                owner,
                name,
                points,
                anchor_indices: None,
                infrastructure: Vec::new(),
                vertical_profile: None,
                section_profiles: Vec::new(),
                track_type,
                design_speed_kmh: None,
            })
        })
        .map(AlignmentId)
    }
    /// Change a newly constructed alignment's design cap, preserving atomic validation.
    pub fn set_alignment_design_speed(
        &mut self,
        id: AlignmentId,
        speed: f64,
    ) -> Result<(), WorldError> {
        let mut candidate = self.snapshot().clone();
        let alignment = candidate
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(WorldError::MissingReference("alignment"))?;
        alignment.design_speed_kmh = Some(speed);
        *self = Self::new(candidate)?;
        Ok(())
    }
    pub fn place_station_on_alignment(
        &mut self,
        owner: CompanyId,
        name: String,
        alignment_id: AlignmentId,
        position: f64,
        length_m: f64,
        width_m: f64,
    ) -> Result<StationId, WorldError> {
        let a = self
            .data
            .alignments
            .iter()
            .find(|a| a.id == alignment_id)
            .ok_or(WorldError::MissingReference("placement.alignment"))?;
        if !position.is_finite()
            || !(0.0..=1.0).contains(&position)
            || !length_m.is_finite()
            || !(20.0..=1000.0).contains(&length_m)
            || length_m > a.length_m()
            || !width_m.is_finite()
            || !(4.0..=100.0).contains(&width_m)
            || owner != a.owner
        {
            return Err(WorldError::InvalidStationPlacement);
        }
        let half = length_m / a.length_m() / 2.0;
        let position = position.clamp(half, 1.0 - half);
        let point = a.point_at(position);
        // Guard only duplicate occupation along this alignment. A different
        // alignment may intentionally carry another platform of a shared station.
        // Keep legacy snapshot loading and existing station resizing unchanged.
        let total = a.length_m();
        if self.data.station_placements.iter().any(|placement| {
            placement.alignment_id == alignment_id
                && (placement.position - position).abs() * total
                    < (placement.length_m + length_m) / 2.0 - 1e-7
        }) {
            return Err(WorldError::StationFootprintOverlap);
        }
        let footprint = super::station_groups::footprint(a, position, length_m, width_m);
        let intersecting: Vec<_> = self
            .data
            .station_placements
            .iter()
            .filter(|p| p.alignment_id != alignment_id)
            .filter_map(|p| {
                let alignment = self
                    .data
                    .alignments
                    .iter()
                    .find(|a| a.id == p.alignment_id && a.owner == owner)?;
                super::station_groups::overlaps(
                    &footprint,
                    &super::station_groups::footprint(alignment, p.position, p.length_m, p.width_m),
                )
                .then_some(p.station_id)
            })
            .collect();
        self.insert(|data, id| {
            data.stations.push(Station {
                id: StationId(id),
                owner,
                name,
                latitude: point.latitude,
                longitude: point.longitude,
            });
            data.station_placements.push(StationPlacement {
                station_id: StationId(id),
                alignment_id,
                position,
                length_m,
                width_m,
            });
            data.join_station_group(StationId(id), &intersecting);
        })
        .map(StationId)
    }
}

impl World {
    pub fn update_station_placement(
        &mut self,
        station_id: StationId,
        length_m: f64,
        width_m: f64,
    ) -> Result<(), WorldError> {
        let mut data = self.snapshot().clone();
        let placement = data
            .station_placements
            .iter_mut()
            .find(|p| p.station_id == station_id)
            .ok_or(WorldError::MissingReference("placement.station"))?;
        let a = data
            .alignments
            .iter()
            .find(|a| a.id == placement.alignment_id)
            .ok_or(WorldError::MissingReference("placement.alignment"))?;
        if !length_m.is_finite()
            || !(20.0..=1000.0).contains(&length_m)
            || length_m > a.length_m()
            || !width_m.is_finite()
            || !(4.0..=100.0).contains(&width_m)
        {
            return Err(WorldError::InvalidStationPlacement);
        }
        let half = length_m / a.length_m() / 2.0;
        placement.position = placement.position.clamp(half, 1.0 - half);
        placement.length_m = length_m;
        placement.width_m = width_m;
        let point = a.point_at(placement.position);
        let station = data
            .stations
            .iter_mut()
            .find(|s| s.id == station_id)
            .ok_or(WorldError::MissingReference("placement.station"))?;
        station.latitude = point.latitude;
        station.longitude = point.longitude;
        self.replace(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_before_stations_and_places_on_curve_atomically() {
        let mut w = World::default();
        let p = w.add_player("P".into()).unwrap();
        let c = w.add_company(p, "C".into()).unwrap();
        let a = w
            .build_alignment(
                c,
                "Line".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.001,
                        longitude: 139.001,
                    },
                    GeoPoint {
                        latitude: 35.003,
                        longitude: 139.002,
                    },
                ],
            )
            .unwrap();
        assert!(w.snapshot().stations.is_empty());
        let s = w
            .place_station_on_alignment(c, "S".into(), a, 0.5, 100.0, 20.0)
            .unwrap();
        assert_eq!(w.snapshot().station_placements[0].station_id, s);
        w.snapshot().validate().unwrap();
        let before = w.clone();
        assert!(w
            .place_station_on_alignment(c, "Bad".into(), a, f64::NAN, 100.0, 20.0)
            .is_err());
        assert!(w
            .place_station_on_alignment(c, "Long".into(), a, 0.5, 1000.0, 20.0)
            .is_err());
        assert_eq!(w, before);
    }
    #[test]
    fn cannot_remove_referenced_alignment_or_forge_placement() {
        let mut w = World::default();
        let p = w.add_player("P".into()).unwrap();
        let c = w.add_company(p, "C".into()).unwrap();
        let a = w
            .build_alignment(
                c,
                "L".into(),
                vec![
                    GeoPoint {
                        latitude: 35.0,
                        longitude: 139.0,
                    },
                    GeoPoint {
                        latitude: 35.01,
                        longitude: 139.0,
                    },
                ],
            )
            .unwrap();
        w.place_station_on_alignment(c, "S".into(), a, 0.0, 100.0, 20.0)
            .unwrap();
        let mut d = w.snapshot().clone();
        d.alignments.clear();
        assert!(w.replace(d).is_err());
        let mut d = w.snapshot().clone();
        d.stations[0].latitude += 0.001;
        assert!(w.replace(d).is_err());
    }
}
