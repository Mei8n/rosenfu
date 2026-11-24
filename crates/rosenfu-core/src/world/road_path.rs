//! Separately typed multi-tile provenance. Never masquerades as one OSM source.
use super::*;
use crate::ofm_evidence::{validate_path_descriptor, PathDescriptor, RawEvidence, TileBinding};
const MAX_PATHS: usize = 32;
const LICENSE: &str = "ODbL-1.0";
const ATTRIBUTION: &str = "© OpenMapTiles © OpenStreetMap contributors";
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentRoadPathProvenance {
    pub alignment_id: AlignmentId,
    pub start_point_index: usize,
    pub end_point_index: usize,
    pub path: PathDescriptor,
    /// Retained arc-length interval on the independently regenerated whole path.
    pub from_position: f64,
    pub to_position: f64,
    pub license: String,
    pub attribution: String,
}
fn generated(
    store: &crate::ofm_evidence::EvidenceStore,
    path: &PathDescriptor,
) -> Result<Vec<GeoPoint>, WorldError> {
    let scoped = store
        .scope(&path.required_binding_keys)
        .map_err(|_| WorldError::InvalidRoadEvidence)?;
    Ok(validate_path_descriptor(&scoped, path)
        .map_err(|_| WorldError::InvalidRoadEvidence)?
        .points
        .into_iter()
        .map(|p| GeoPoint {
            longitude: p[0],
            latitude: p[1],
        })
        .collect())
}
fn same_points(a: &[GeoPoint], b: &[GeoPoint]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            (a.latitude - b.latitude).abs() <= 1e-10 && (a.longitude - b.longitude).abs() <= 1e-10
        })
}
impl WorldDraft {
    /// A writer must select the new save envelope before emitting this proof.
    /// Inspect parked descriptors, too, even though the current authoring
    /// transaction doesn't let a street-tram preview become a parked plan.
    pub fn carries_street_tram_proof(&self) -> bool {
        self.has_road_descriptor(PathDescriptor::carries_street_tram_proof)
    }

    pub fn carries_adjacent_street_tram_proof(&self) -> bool {
        self.has_road_descriptor(PathDescriptor::carries_adjacent_street_tram_proof)
    }

    pub fn carries_fractional_bus_proof(&self) -> bool {
        self.ofm_gameplay_bus_routes
            .values()
            .any(|binding| binding.same_atom_stops.is_some())
    }

    pub fn carries_chain_street_tram_proof(&self) -> bool {
        self.has_road_descriptor(PathDescriptor::carries_chain_street_tram_proof)
    }

    pub fn carries_general_fractional_bus_proof(&self) -> bool {
        self.ofm_gameplay_bus_routes
            .values()
            .any(|binding| binding.fractional_terminals.is_some())
    }

    pub fn carries_general_fractional_bus_stops_proof(&self) -> bool {
        self.ofm_gameplay_bus_routes.values().any(|binding| {
            binding.fractional_terminals.is_some() && !binding.interior_stops.is_empty()
        })
    }

    fn has_road_descriptor(&self, check: fn(&PathDescriptor) -> bool) -> bool {
        self.road_path_provenance
            .iter()
            .any(|range| check(&range.path))
            || self.construction_plans.as_ref().is_some_and(|p| {
                p.parked_paths
                    .iter()
                    .any(|path| path.ofm_path.as_ref().is_some_and(check))
            })
    }

    /// Saved parked paths share the budget and immutable source evidence of
    /// the already-built paths. Used by both whole-world and metadata edits.
    pub(super) fn validate_parked_road_paths(
        &self,
        plans: Option<&ConstructionPlans>,
    ) -> Result<(), WorldError> {
        let plan_paths = plans.map_or(0, |p| {
            p.parked_paths
                .iter()
                .filter(|p| p.ofm_path.is_some())
                .count()
        });
        if self.road_path_provenance.len() + plan_paths > MAX_PATHS {
            return Err(WorldError::InvalidRoadEvidence);
        }
        if let Some(plans) = plans {
            for p in &plans.parked_paths {
                if let Some(path) = &p.ofm_path {
                    let actual = generated(&self.runtime_road_evidence, path)?;
                    if path.carries_street_tram_proof()
                        || p.reference.is_some()
                        || p.edited_geometry.is_some()
                        || !same_points(&p.points, &actual)
                        || !same_points(&p.line, &actual)
                    {
                        return Err(WorldError::InvalidConstructionPlans);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_road_path_provenance(&self) -> Result<(), WorldError> {
        self.validate_parked_road_paths(self.construction_plans.as_ref())?;
        let mut ranges: std::collections::BTreeMap<AlignmentId, Vec<(usize, usize)>> =
            std::collections::BTreeMap::new();
        for p in &self.alignment_provenance {
            ranges
                .entry(p.alignment_id)
                .or_default()
                .push((p.start_point_index, p.end_point_index));
        }
        for p in &self.road_path_provenance {
            let a = self
                .alignments
                .iter()
                .find(|a| a.id == p.alignment_id)
                .ok_or(WorldError::InvalidProvenance)?;
            if p.start_point_index >= p.end_point_index
                || p.end_point_index >= a.points.len()
                || p.license != LICENSE
                || p.attribution != ATTRIBUTION
                || !p.from_position.is_finite()
                || !p.to_position.is_finite()
                || p.from_position < 0.
                || p.to_position > 1.
                || p.from_position >= p.to_position
            {
                return Err(WorldError::InvalidProvenance);
            }
            if p.path.carries_street_tram_proof()
                && (a.track_type != TrackType::Tram
                    || a.vertical_profile.is_some()
                    || !a.section_profiles.is_empty()
                    || a.effective_infrastructure()
                        .iter()
                        .any(|s| s.level != InfrastructureLevel::Ground))
            {
                return Err(WorldError::InvalidProvenance);
            }
            let original = generated(&self.runtime_road_evidence, &p.path)?;
            if !super::provenance::slice_matches(
                &ProvenanceGeometrySlice {
                    original_points: original,
                    from_position: p.from_position,
                    to_position: p.to_position,
                },
                &a.points[p.start_point_index..=p.end_point_index],
            ) {
                return Err(WorldError::InvalidProvenance);
            }
            let prior = ranges.entry(p.alignment_id).or_default();
            if prior
                .iter()
                .any(|(s, e)| p.start_point_index < *e && *s < p.end_point_index)
            {
                return Err(WorldError::InvalidProvenance);
            }
            prior.push((p.start_point_index, p.end_point_index));
        }
        Ok(())
    }
    pub(super) fn preserve_road_paths(&self, old: &Self) -> Result<(), WorldError> {
        if self.runtime_road_evidence != old.runtime_road_evidence {
            return Err(WorldError::ProvenanceEditUnsupported);
        }
        for p in &old.road_path_provenance {
            if let Some(a) = self.alignments.iter().find(|a| a.id == p.alignment_id) {
                if !self.road_path_provenance.contains(p)
                    || old
                        .alignments
                        .iter()
                        .find(|o| o.id == a.id)
                        .map(|o| &o.points)
                        != Some(&a.points)
                {
                    return Err(WorldError::ProvenanceEditUnsupported);
                }
            }
        }
        if self
            .road_path_provenance
            .iter()
            .any(|p| !old.road_path_provenance.contains(p))
        {
            return Err(WorldError::ProvenanceEditUnsupported);
        }
        Ok(())
    }
    pub(super) fn split_road_path_provenance(
        &mut self,
        id: AlignmentId,
        suffix: AlignmentId,
        split: usize,
        points: &[GeoPoint],
    ) -> Result<(), WorldError> {
        let old = std::mem::take(&mut self.road_path_provenance);
        for p in old {
            if p.alignment_id != id || p.end_point_index <= split {
                self.road_path_provenance.push(p);
                continue;
            }
            if p.start_point_index >= split {
                let mut p = p;
                p.alignment_id = suffix;
                p.start_point_index -= split;
                p.end_point_index -= split;
                self.road_path_provenance.push(p);
                continue;
            }
            let length = |a: usize, b: usize| {
                points[a..=b]
                    .windows(2)
                    .map(|e| super::alignment::distance(e[0], e[1]))
                    .sum::<f64>()
            };
            let total = length(p.start_point_index, p.end_point_index);
            if total <= 0. {
                return Err(WorldError::InvalidProvenance);
            }
            let mid = p.from_position
                + (p.to_position - p.from_position) * length(p.start_point_index, split) / total;
            let mut left = p.clone();
            left.end_point_index = split;
            left.to_position = mid;
            let mut right = p;
            right.alignment_id = suffix;
            right.start_point_index = 0;
            right.end_point_index -= split;
            right.from_position = mid;
            self.road_path_provenance.extend([left, right]);
        }
        Ok(())
    }
}
impl World {
    fn can_mix_fractional_with_street_tram(&self) -> bool {
        self.operation()
            .is_none_or(|op| op.runtime.admission_model() as u32 == 2)
    }

    /// Test the saved capability before doing a preview search or a write.
    pub fn can_author_fractional_bus(&self) -> bool {
        #[cfg(feature = "standard-periodic-rail")]
        if self.has_standard_timetable() {
            return false;
        }
        let data = self.snapshot();
        !data.carries_adjacent_street_tram_proof()
            && !data.carries_chain_street_tram_proof()
            && (!data.carries_street_tram_proof() || self.can_mix_fractional_with_street_tram())
            && !self
                .operation()
                .is_some_and(|op| op.runtime.admission_model() as u32 > 7)
    }

    pub fn can_author_general_fractional_bus(&self) -> bool {
        self.can_mix_fractional_with_street_tram()
    }

    pub fn can_author_street_tram_with_bus_capabilities(
        &self,
        guide: crate::ofm_evidence::AuthoredTramProof<'_>,
    ) -> bool {
        use crate::ofm_evidence::AuthoredTramProof;
        let data = self.snapshot();
        let fixed = self.can_mix_fractional_with_street_tram();
        match guide {
            AuthoredTramProof::SingleOrAdjacent(guide) => {
                (!data.carries_general_fractional_bus_proof() || fixed)
                    && (!data.carries_fractional_bus_proof()
                        || (guide.adjacent_end.is_none()
                            && !data.carries_adjacent_street_tram_proof()
                            && !data.carries_chain_street_tram_proof()
                            && self.can_author_fractional_bus()))
            }
            AuthoredTramProof::Chain(_) => !data.carries_fractional_bus_proof() && fixed,
        }
    }

    /// An ordinary OFM Bus edit owns only its addressed raw tiles and the
    /// route reconstructed from it. Never permits arbitrary draft replacement.
    pub fn candidate_with_gameplay_ofm_bus(
        &self,
        owner: CompanyId,
        route_id: String,
        name: String,
        path: crate::ofm_evidence::BusPathBinding,
        cached: Vec<(RawEvidence, TileBinding)>,
    ) -> Result<Self, crate::road_transport::RoadTransportError> {
        if (path.fractional_terminals.is_some() && !self.can_author_general_fractional_bus())
            || (path.same_atom_stops.is_some() && !self.can_author_fractional_bus())
        {
            return Err(crate::road_transport::RoadTransportError::InvalidData);
        }
        let mut candidate = self.clone();
        candidate.data = self
            .snapshot()
            .clone()
            .admit_gameplay_ofm_bus_tiles(&path, cached)?
            .put_ofm_gameplay_bus(owner, route_id, name, path)?
            .into();
        candidate
            .data
            .validate()
            .map_err(|_| crate::road_transport::RoadTransportError::InvalidData)?;
        Ok(candidate)
    }
}

impl World {
    /// Return a detached candidate containing the single tile and its exact
    /// street-tram derivation. The editor uses this only with its ordinary
    /// construction transaction and can discard it on any later rejection.
    pub fn staged_street_tram_for_construction<'a>(
        &self,
        guide: impl Into<crate::ofm_evidence::AuthoredTramProof<'a>>,
        cached: Vec<(RawEvidence, TileBinding)>,
    ) -> Result<(Self, crate::ofm_evidence::DerivedPath), crate::road_transport::RoadTransportError>
    {
        let draft = self.snapshot();
        let guide = guide.into();
        if !self.can_author_street_tram_with_bus_capabilities(guide) {
            return Err(crate::road_transport::RoadTransportError::InvalidData);
        }
        let (draft, path) = draft.clone().stage_authored_street_tram(guide, cached)?;
        let mut candidate = self.clone();
        candidate.data = draft.into();
        Ok((candidate, path))
    }

    /// Park light plans and raw evidence in one candidate. Supplied preview points
    /// are replaced by regenerated backend points, not promoted to trusted geometry.
    pub fn stage_ofm_construction_plans(
        &mut self,
        mut plans: ConstructionPlans,
        raw: Vec<RawEvidence>,
        bindings: Vec<TileBinding>,
    ) -> Result<(), WorldError> {
        if plans
            .parked_paths
            .iter()
            .filter(|p| p.ofm_path.is_some())
            .count()
            + self.data.road_path_provenance.len()
            > MAX_PATHS
        {
            return Err(WorldError::InvalidRoadEvidence);
        }
        let store = self
            .data
            .runtime_road_evidence
            .with_pinned_additions(raw, bindings)
            .map_err(|_| WorldError::InvalidRoadEvidence)?;
        for p in &mut plans.parked_paths {
            if let Some(path) = &p.ofm_path {
                if p.reference.is_some() || p.edited_geometry.is_some() || !p.redo.is_empty() {
                    return Err(WorldError::InvalidConstructionPlans);
                }
                let points = generated(&store, path)?;
                p.points = points.clone();
                p.line = points;
            }
        }
        let mut draft = self.snapshot().clone();
        draft.road_evidence_descriptors = store.descriptors();
        draft.runtime_road_evidence = store;
        draft.construction_plans = Some(plans);
        draft.validate()?;
        self.data = draft.into();
        Ok(())
    }
    pub fn build_alignment_from_road_path(
        &mut self,
        owner: CompanyId,
        name: String,
        track_type: TrackType,
        path: PathDescriptor,
    ) -> Result<AlignmentId, WorldError> {
        let points = generated(&self.data.runtime_road_evidence, &path)?;
        let end = points.len() - 1;
        let mut candidate = self.clone();
        let id = candidate.build_alignment_typed(owner, name, points, track_type)?;
        candidate
            .data_mut()
            .road_path_provenance
            .push(AlignmentRoadPathProvenance {
                alignment_id: id,
                start_point_index: 0,
                end_point_index: end,
                path,
                from_position: 0.,
                to_position: 1.,
                license: LICENSE.into(),
                attribution: ATTRIBUTION.into(),
            });
        candidate.data.validate()?;
        *self = candidate;
        Ok(id)
    }
    pub fn extend_alignment_from_road_path(
        &mut self,
        owner: CompanyId,
        id: AlignmentId,
        endpoint: AlignmentEndpoint,
        mut path: PathDescriptor,
    ) -> Result<(), WorldError> {
        if path.carries_street_tram_proof() {
            return Err(WorldError::ProvenanceEditUnsupported);
        }
        let points = generated(&self.data.runtime_road_evidence, &path)?;
        let count = points.len() - 1;
        let old = self
            .data
            .alignments
            .iter()
            .find(|a| a.id == id)
            .ok_or(WorldError::MissingReference("alignment"))?
            .points
            .len();
        let mut candidate = self.clone();
        candidate.extend_alignment(owner, id, endpoint, points)?;
        let start = match endpoint {
            AlignmentEndpoint::End => old - 1,
            AlignmentEndpoint::Start => {
                let rebuilt = crate::ofm_evidence::reconstruct_path(
                    &candidate
                        .data
                        .runtime_road_evidence
                        .scope(&path.required_binding_keys)
                        .map_err(|_| WorldError::InvalidRoadEvidence)?,
                    &path.required_binding_keys,
                    &path.end,
                    &path.start,
                )
                .map_err(|_| WorldError::InvalidRoadEvidence)?;
                path = rebuilt.descriptor;
                0
            }
        };
        candidate
            .data_mut()
            .road_path_provenance
            .push(AlignmentRoadPathProvenance {
                alignment_id: id,
                start_point_index: start,
                end_point_index: start + count,
                path,
                from_position: 0.,
                to_position: 1.,
                license: LICENSE.into(),
                attribution: ATTRIBUTION.into(),
            });
        candidate.data.validate()?;
        *self = candidate;
        Ok(())
    }
}
