//! Atomic splitting at an existing construction anchor for explicit turnouts.
use super::construction_plans::ConstructionExtension;
use super::*;

impl World {
    /// Split at the exact projected position of a sampled geometric segment.
    /// The clicked position is never replaced by the nearest construction anchor.
    pub fn split_alignment_at_position(
        &mut self,
        owner: CompanyId,
        id: AlignmentId,
        segment: usize,
        fraction: f64,
    ) -> Result<AlignmentId, WorldError> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(WorldError::InvalidAlignment);
        }
        let mut draft = self.data.clone();
        let original_points = draft
            .alignments
            .iter()
            .find(|a| a.id == id && a.owner == owner)
            .ok_or(WorldError::MissingReference("junction.alignment"))?
            .points
            .clone();
        let alignment = draft
            .alignments
            .iter_mut()
            .find(|a| a.id == id && a.owner == owner)
            .ok_or(WorldError::MissingReference("junction.alignment"))?;
        let next = segment.checked_add(1).ok_or(WorldError::InvalidAlignment)?;
        if next >= alignment.points.len() {
            return Err(WorldError::InvalidAlignment);
        }
        let mut anchors = alignment
            .anchor_indices
            .clone()
            .unwrap_or_else(|| vec![0, alignment.points.len() - 1]);
        let mut infrastructure = alignment.effective_infrastructure();
        let mut section_profiles = alignment.section_profiles.clone();
        let point_index = if fraction == 0.0 {
            segment
        } else if fraction == 1.0 {
            next
        } else {
            let a = alignment.points[segment];
            let b = alignment.points[next];
            let point = GeoPoint {
                latitude: a.latitude + (b.latitude - a.latitude) * fraction,
                longitude: a.longitude + (b.longitude - a.longitude) * fraction,
            };
            alignment.points.insert(next, point);
            for index in &mut anchors {
                if *index >= next {
                    *index += 1;
                }
            }
            next
        };
        if point_index == 0 || point_index + 1 == alignment.points.len() {
            return Err(WorldError::InvalidAlignment);
        }
        let anchor = match anchors.binary_search(&point_index) {
            Ok(index) => index,
            Err(index) => {
                let attr = *infrastructure
                    .get(index - 1)
                    .ok_or(WorldError::InvalidAlignment)?;
                if !section_profiles.is_empty() {
                    let length = |start: usize, end: usize| {
                        alignment.points[start..=end]
                            .windows(2)
                            .map(|p| super::alignment::distance(p[0], p[1]))
                            .sum::<f64>()
                    };
                    let fraction = length(anchors[index - 1], point_index)
                        / length(anchors[index - 1], anchors[index]);
                    let old = section_profiles[index - 1];
                    section_profiles[index - 1] =
                        old.clip(0., fraction).ok_or(WorldError::InvalidAlignment)?;
                    section_profiles.insert(
                        index,
                        old.clip(fraction, 1.).ok_or(WorldError::InvalidAlignment)?,
                    );
                }
                anchors.insert(index, point_index);
                infrastructure.insert(index, attr);
                index
            }
        };
        alignment.anchor_indices = Some(anchors);
        alignment.infrastructure = infrastructure;
        alignment.section_profiles = section_profiles;
        if fraction > 0.0 && fraction < 1.0 {
            for provenance in draft
                .alignment_provenance
                .iter_mut()
                .filter(|p| p.alignment_id == id)
            {
                if provenance.start_point_index < next
                    && next <= provenance.end_point_index
                    && provenance.derived_slice.is_none()
                {
                    provenance.derived_slice = Some(ProvenanceGeometrySlice {
                        original_points: original_points
                            [provenance.start_point_index..=provenance.end_point_index]
                            .to_vec(),
                        from_position: 0.0,
                        to_position: 1.0,
                    });
                }
                if provenance.start_point_index >= next {
                    provenance.start_point_index += 1;
                }
                if provenance.end_point_index >= next {
                    provenance.end_point_index += 1;
                }
            }
        }
        let mut candidate = Self::new(draft)?;
        let suffix = candidate.split_alignment_at_anchor(owner, id, anchor)?;
        *self = candidate;
        Ok(suffix)
    }
    /// Split a user-drawn alignment without removing any physical section.
    /// The prefix keeps its ID. Station IDs, service leg keys and stop references
    /// are preserved; crossing path segments become explicit connected corridors.
    /// Vertical profiles and source lineage are clipped exactly; straddling platforms are rejected.
    pub fn split_alignment_at_anchor(
        &mut self,
        owner: CompanyId,
        id: AlignmentId,
        anchor: usize,
    ) -> Result<AlignmentId, WorldError> {
        let mut draft = self.data.clone();
        let index = draft
            .alignments
            .iter()
            .position(|a| a.id == id && a.owner == owner)
            .ok_or(WorldError::MissingReference("junction.alignment"))?;
        let original = draft.alignments[index].clone();
        let boundaries = original
            .anchor_indices
            .clone()
            .ok_or(WorldError::InvalidAlignment)?;
        if anchor == 0 || anchor + 1 >= boundaries.len() {
            return Err(WorldError::InvalidAlignment);
        }
        let split = boundaries[anchor];
        let mut prefix = original.clone();
        prefix.points = original.points[..=split].to_vec();
        prefix.anchor_indices = Some(boundaries[..=anchor].to_vec());
        prefix.infrastructure = original.effective_infrastructure()[..anchor].to_vec();
        if !original.section_profiles.is_empty() {
            prefix.section_profiles = original.section_profiles[..anchor].to_vec();
        }
        let mut suffix = original.clone();
        suffix.id = AlignmentId(draft.next_id);
        draft.next_id = draft
            .next_id
            .checked_add(1)
            .ok_or(WorldError::IdExhausted)?;
        suffix.points = original.points[split..].to_vec();
        suffix.anchor_indices = Some(boundaries[anchor..].iter().map(|i| i - split).collect());
        suffix.infrastructure = original.effective_infrastructure()[anchor..].to_vec();
        if !original.section_profiles.is_empty() {
            suffix.section_profiles = original.section_profiles[anchor..].to_vec();
        }
        let cut = prefix.length_m();
        let total = original.length_m();
        if let Some(profile) = original.vertical_profile {
            prefix.vertical_profile = profile.clip(0.0, cut / total);
            suffix.vertical_profile = profile.clip(cut / total, 1.0);
            if prefix.vertical_profile.is_none() || suffix.vertical_profile.is_none() {
                return Err(WorldError::InvalidAlignment);
            }
        }
        let suffix_id = suffix.id;
        let old_provenance = std::mem::take(&mut draft.alignment_provenance);
        for provenance in old_provenance {
            if provenance.alignment_id != id {
                draft.alignment_provenance.push(provenance);
                continue;
            }
            if provenance.end_point_index <= split {
                draft.alignment_provenance.push(provenance);
                continue;
            }
            if provenance.start_point_index >= split {
                let mut p = provenance;
                p.alignment_id = suffix_id;
                p.start_point_index -= split;
                p.end_point_index -= split;
                draft.alignment_provenance.push(p);
                continue;
            }
            let span = &original.points[provenance.start_point_index..=provenance.end_point_index];
            let span_length: f64 = span
                .windows(2)
                .map(|p| super::alignment::distance(p[0], p[1]))
                .sum();
            let left_length: f64 = original.points[provenance.start_point_index..=split]
                .windows(2)
                .map(|p| super::alignment::distance(p[0], p[1]))
                .sum();
            let lineage = provenance
                .derived_slice
                .clone()
                .unwrap_or(ProvenanceGeometrySlice {
                    original_points: span.to_vec(),
                    from_position: 0.,
                    to_position: 1.,
                });
            let mid = lineage.from_position
                + (lineage.to_position - lineage.from_position) * left_length / span_length;
            let mut left = provenance.clone();
            left.end_point_index = split;
            left.derived_slice = Some(ProvenanceGeometrySlice {
                to_position: mid,
                ..lineage.clone()
            });
            let mut right = provenance;
            right.alignment_id = suffix_id;
            right.start_point_index = 0;
            right.end_point_index -= split;
            right.derived_slice = Some(ProvenanceGeometrySlice {
                from_position: mid,
                ..lineage
            });
            draft.alignment_provenance.extend([left, right]);
        }
        for placement in draft
            .station_placements
            .iter_mut()
            .filter(|p| p.alignment_id == id)
        {
            let centre = placement.position * total;
            if (centre - cut).abs() < placement.length_m / 2.0 {
                return Err(WorldError::InvalidStationPlacement);
            }
            if centre > cut {
                placement.alignment_id = suffix_id;
                placement.position = (centre - cut) / suffix.length_m();
            } else {
                placement.position = centre / cut;
            }
        }
        let prefix_end = AlignmentEndpointRef {
            alignment_id: id,
            endpoint: AlignmentEndpoint::End,
        };
        let suffix_start = AlignmentEndpointRef {
            alignment_id: suffix_id,
            endpoint: AlignmentEndpoint::Start,
        };
        let remap = |end: &mut AlignmentEndpointRef| {
            if end.alignment_id == id && end.endpoint == AlignmentEndpoint::End {
                end.alignment_id = suffix_id;
            }
        };
        for link in &mut draft.alignment_connections {
            remap(&mut link.a);
            remap(&mut link.b);
        }
        draft.alignment_connections.push(AlignmentConnection {
            a: prefix_end,
            b: suffix_start,
        });
        let placements = draft.station_placements.clone();
        let rewrite = |segment: &mut RouteSegment| -> Result<(), WorldError> {
            let (from, to, mut links) = match segment {
                RouteSegment::Alignment { from, to, .. } => (*from, *to, vec![]),
                RouteSegment::ConnectedAlignments {
                    from,
                    to,
                    connections,
                } => (*from, *to, connections.clone()),
            };
            let find = |station| {
                placements
                    .iter()
                    .find(|p| p.station_id == station)
                    .map(|p| p.alignment_id)
                    .ok_or(WorldError::InvalidRoutePath)
            };
            let mut current = find(from)?;
            let target = find(to)?;
            let mut result = vec![];
            let bridge = |a, b| -> Result<AlignmentConnection, WorldError> {
                if a == id && b == suffix_id {
                    Ok(AlignmentConnection {
                        a: prefix_end,
                        b: suffix_start,
                    })
                } else if a == suffix_id && b == id {
                    Ok(AlignmentConnection {
                        a: suffix_start,
                        b: prefix_end,
                    })
                } else {
                    Err(WorldError::InvalidRoutePath)
                }
            };
            for link in &mut links {
                remap(&mut link.a);
                remap(&mut link.b);
                if current != link.a.alignment_id {
                    result.push(bridge(current, link.a.alignment_id)?);
                }
                result.push(link.clone());
                current = link.b.alignment_id;
            }
            if current != target {
                result.push(bridge(current, target)?);
            }
            *segment = if result.is_empty() {
                RouteSegment::Alignment {
                    alignment_id: current,
                    from,
                    to,
                }
            } else {
                RouteSegment::ConnectedAlignments {
                    from,
                    to,
                    connections: result,
                }
            };
            Ok(())
        };
        for path in &mut draft.route_paths {
            for segment in &mut path.segments {
                rewrite(segment)?;
            }
        }
        for path in &mut draft.service_paths {
            for leg in &mut path.legs {
                rewrite(&mut leg.segment)?;
            }
        }
        if let Some(plans) = &mut draft.construction_plans {
            if plans
                .planned_stations
                .iter()
                .any(|p| p.alignment_id == Some(id))
            {
                return Err(WorldError::InvalidConstructionPlans);
            }
            let fix = |end: &mut Option<ConstructionExtension>| {
                if let Some(end) = end {
                    if end.alignment_id == id && end.endpoint == AlignmentEndpoint::End {
                        end.alignment_id = suffix_id;
                    }
                }
            };
            fix(&mut plans.draft_extension);
            fix(&mut plans.draft_destination);
            fix(&mut plans.draft_redo_destination);
            for path in &mut plans.parked_paths {
                fix(&mut path.extension);
                fix(&mut path.destination);
                fix(&mut path.redo_destination);
            }
        }
        draft.alignments[index] = prefix;
        draft.alignments.push(suffix);
        let candidate = Self::new(draft)?;
        for service in &self.data.services {
            if candidate.service_path_occurrences(service.id)?
                != self.service_path_occurrences(service.id)?
            {
                return Err(WorldError::ServicePathEditUnsupported);
            }
        }
        *self = candidate;
        Ok(suffix_id)
    }
}
