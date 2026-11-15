//! Saved editor drafts. Incomplete geometry is allowed; construction validates it later.
use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionPlans {
    #[serde(default)]
    pub infrastructure: SectionInfrastructure,
    pub version: u32,
    pub plans: Vec<ConstructionPlan>,
    pub parked_paths: Vec<ConstructionPath>,
    pub active_path_id: u64,
    pub active_plan_id: u64,
    pub draft_points: Vec<GeoPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_edited_geometry: Option<EditedPlanGeometry>,
    pub redo_points: Vec<GeoPoint>,
    pub draft_extension: Option<ConstructionExtension>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_destination: Option<ConstructionExtension>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_redo_destination: Option<ConstructionExtension>,
    pub track_type: TrackType,
    pub design_speed_kmh: f64,
    pub road_following: bool,
    pub planned_stations: Vec<ConstructionStation>,
    pub draft_finished: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionPlan {
    pub id: u64,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionExtension {
    pub alignment_id: AlignmentId,
    pub endpoint: AlignmentEndpoint,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionReference {
    pub name: String,
    pub points: Vec<GeoPoint>,
    pub source_record: SourceRecord,
    pub provenance: AlignmentProvenanceInput,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionPath {
    #[serde(default)]
    pub infrastructure: SectionInfrastructure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_geometry: Option<EditedPlanGeometry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<ConstructionReference>,
    pub extension: Option<ConstructionExtension>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<ConstructionExtension>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redo_destination: Option<ConstructionExtension>,
    pub id: u64,
    pub plan_id: u64,
    pub name: String,
    pub points: Vec<GeoPoint>,
    pub redo: Vec<GeoPoint>,
    pub line: Vec<GeoPoint>,
    pub design_speed_kmh: f64,
    pub track_type: TrackType,
    pub road_following: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstructionStation {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment_id: Option<AlignmentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<u64>,
    pub name: String,
    pub point: GeoPoint,
    pub length_m: f64,
}
/// Versioned sampled edit; never reconstruct old plan shapes from moved anchors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditedPlanGeometry {
    pub version: u32,
    pub anchors: Vec<GeoPoint>,
    pub line: Vec<GeoPoint>,
}
impl EditedPlanGeometry {
    fn valid(&self, current: &[GeoPoint]) -> bool {
        if self.version != 1
            || self.anchors.len() < 2
            || self.anchors.len() > 128
            || self.line.len() < 2
            || self.line.len() > 4096
            || !valid_points(&self.anchors)
            || !valid_points(&self.line)
            || current.iter().zip(&self.anchors).any(|(a, b)| a != b)
        {
            return false;
        }
        let mut start = 0;
        for anchor in &self.anchors {
            let Some(offset) = self.line[start..].iter().position(|p| p == anchor) else {
                return false;
            };
            start += offset + 1;
        }
        self.line.first() == self.anchors.first() && self.line.last() == self.anchors.last()
    }
}
fn valid_id(id: u64) -> bool {
    id > 0 && id < 9_007_199_254_740_991
}
fn valid_points(points: &[GeoPoint]) -> bool {
    points.len() <= 100_000
        && points.iter().all(|p| {
            p.latitude.is_finite()
                && p.longitude.is_finite()
                && (20.0..=46.0).contains(&p.latitude)
                && (122.0..=154.0).contains(&p.longitude)
        })
}
impl ConstructionPlans {
    pub fn validate(&self) -> Result<(), WorldError> {
        let invalid = WorldError::InvalidConstructionPlans;
        if !self.infrastructure.is_valid()
            || self
                .parked_paths
                .iter()
                .any(|path| !path.infrastructure.is_valid())
            || self.version != 1
            || self.plans.is_empty()
            || self.plans.len() > 10_000
            || self.parked_paths.len() > 10_000
            || self.planned_stations.len() > 10_000
            || !valid_id(self.active_path_id)
            || !self.design_speed_kmh.is_finite()
            || !(1.0..=1000.0).contains(&self.design_speed_kmh)
            || !valid_points(&self.draft_points)
            || !valid_points(&self.redo_points)
        {
            return Err(invalid);
        }
        if self
            .draft_edited_geometry
            .as_ref()
            .is_some_and(|g| !g.valid(&self.draft_points))
        {
            return Err(invalid);
        }
        if self.draft_destination.as_ref().is_some_and(|d| {
            self.draft_extension
                .as_ref()
                .is_none_or(|e| e.alignment_id == d.alignment_id && e.endpoint == d.endpoint)
                || self.draft_points.len() < 2
        }) {
            return Err(invalid);
        }
        let mut plans = BTreeSet::new();
        for plan in &self.plans {
            if !valid_id(plan.id) || !plans.insert(plan.id) || plan.name.len() > 4096 {
                return Err(invalid);
            }
        }
        if !plans.contains(&self.active_plan_id) {
            return Err(invalid);
        }
        let mut paths = BTreeSet::from([self.active_path_id]);
        for path in &self.parked_paths {
            if path.destination.as_ref().is_some_and(|d| {
                path.extension
                    .as_ref()
                    .is_none_or(|e| e.alignment_id == d.alignment_id && e.endpoint == d.endpoint)
                    || path.points.len() < 2
            }) {
                return Err(invalid);
            }
            if path
                .edited_geometry
                .as_ref()
                .is_some_and(|g| path.reference.is_some() || !g.valid(&path.points))
            {
                return Err(invalid);
            }
            if !valid_id(path.id)
                || !paths.insert(path.id)
                || !plans.contains(&path.plan_id)
                || path.name.len() > 4096
                || !valid_points(&path.points)
                || !valid_points(&path.redo)
                || !valid_points(&path.line)
                || !path.design_speed_kmh.is_finite()
                || !(1.0..=1000.0).contains(&path.design_speed_kmh)
            {
                return Err(invalid);
            }
            if let Some(reference) = &path.reference {
                if reference.name.len() > 4096
                    || !valid_points(&reference.points)
                    || !reference.source_record.valid()
                    || !reference.provenance.valid()
                    || reference.source_record.key != reference.provenance.source_key
                {
                    return Err(invalid);
                }
            }
        }
        let mut stations = BTreeSet::new();
        for station in &self.planned_stations {
            let Some(id) = station
                .id
                .strip_prefix("planned:")
                .and_then(|id| id.parse::<u64>().ok())
            else {
                return Err(invalid);
            };
            if !valid_id(id)
                || station.id != format!("planned:{id}")
                || !stations.insert(id)
                || match (station.path_id, station.alignment_id) {
                    (Some(path), None) => {
                        let owner = if path == self.active_path_id {
                            Some(self.active_plan_id)
                        } else {
                            self.parked_paths
                                .iter()
                                .find(|p| p.id == path)
                                .map(|p| p.plan_id)
                        };
                        owner.is_none() || station.plan_id.is_some_and(|id| Some(id) != owner)
                    }
                    (None, Some(_)) => station.plan_id.is_none_or(|id| !plans.contains(&id)),
                    _ => true,
                }
                || station.name.len() > 4096
                || !valid_points(&[station.point])
                || !station.length_m.is_finite()
                || station.length_m <= 0.0
                || station.length_m > 10_000.0
            {
                return Err(invalid);
            }
        }
        Ok(())
    }
}
