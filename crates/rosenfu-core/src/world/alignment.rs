use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeoPoint {
    pub latitude: f64,
    pub longitude: f64,
}
/// Infrastructure class; performance and construction rules are not yet simulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackType {
    Normal,
    HighSpeed,
    Tram,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alignment {
    pub id: AlignmentId,
    pub owner: CompanyId,
    pub name: String,
    pub points: Vec<GeoPoint>,
    pub track_type: TrackType,
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

fn distance(a: GeoPoint, b: GeoPoint) -> f64 {
    let x = (b.longitude - a.longitude).to_radians()
        * ((a.latitude + b.latitude) / 2.0).to_radians().cos();
    let y = (b.latitude - a.latitude).to_radians();
    6_371_000.0 * x.hypot(y)
}
impl Alignment {
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
        let mut placed = BTreeSet::new();
        for p in &self.station_placements {
            let a = self
                .alignments
                .iter()
                .find(|a| a.id == p.alignment_id)
                .ok_or(WorldError::MissingReference("placement.alignment"))?;
            let s = self
                .stations
                .iter()
                .find(|s| s.id == p.station_id)
                .ok_or(WorldError::MissingReference("placement.station"))?;
            if !placed.insert(p.station_id)
                || a.owner != s.owner
                || !p.position.is_finite()
                || !(0.0..=1.0).contains(&p.position)
                || !p.length_m.is_finite()
                || !(20.0..=1000.0).contains(&p.length_m)
                || !p.width_m.is_finite()
                || !(4.0..=100.0).contains(&p.width_m)
                || p.length_m > a.length_m() + 0.001
            {
                return Err(WorldError::InvalidStationPlacement);
            }
            let half = p.length_m / a.length_m() / 2.0;
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
                track_type,
            })
        })
        .map(AlignmentId)
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
        let mut data = self.data.clone();
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
