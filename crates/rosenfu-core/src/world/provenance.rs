//! Untrusted provenance snapshots, not authenticity or license-compliance proofs.
use super::*;
const MAX_SOURCES: usize = 1024;
const MAX_PROVENANCE: usize = 10000;
const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    pub key: String,
    pub dataset_id: String,
    pub revision: String,
    pub graph_sha256: String,
    pub source_sha256: String,
    pub source: String,
    pub source_date: Option<String>,
    pub license: String,
    pub attribution: String,
    pub conversion_format_version: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceMethod {
    OsmSegmentCopyV1,
    OsmWaySliceV1,
    RailGraphRouteV1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceMode {
    Rail,
    Road,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentProvenanceInput {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network_path: Vec<RailGraphSpan>,
    pub source_key: String,
    pub method: ProvenanceMethod,
    pub osm_way_id: String,
    pub mode: ReferenceMode,
    pub start_segment_index: u64,
    pub start_fraction: f64,
    pub end_segment_index: u64,
    pub end_fraction: f64,
}
/// Versioned supplier chain identity; this is deliberately not an OSM way ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RailGraphSpan {
    pub region: String,
    pub chain_id: u32,
    pub from_fraction: f64,
    pub to_fraction: f64,
}
/// Explicit lineage for a geometric subset. The original OSM segment range is
/// retained unchanged; we do not guess correspondence to missing raw way nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceGeometrySlice {
    pub original_points: Vec<GeoPoint>,
    pub from_position: f64,
    pub to_position: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_slice: Option<ProvenanceGeometrySlice>,
    pub alignment_id: AlignmentId,
    pub source: AlignmentProvenanceInput,
    /// Inclusive point range in the current alignment; outside this range is not attributed.
    pub start_point_index: usize,
    pub end_point_index: usize,
}
fn geometry_positions(points: &[GeoPoint]) -> (Vec<f64>, f64) {
    let mut positions = vec![0.];
    let mut length = 0.;
    for pair in points.windows(2) {
        length += super::alignment::distance(pair[0], pair[1]);
        positions.push(length);
    }
    if length > 0. {
        for p in &mut positions {
            *p /= length;
        }
    }
    (positions, length)
}
fn geometry_point(points: &[GeoPoint], positions: &[f64], t: f64) -> GeoPoint {
    for i in 0..points.len() - 1 {
        if t <= positions[i + 1] {
            let width = positions[i + 1] - positions[i];
            let f = if width > 0. {
                (t - positions[i]) / width
            } else {
                0.
            };
            return GeoPoint {
                latitude: points[i].latitude + (points[i + 1].latitude - points[i].latitude) * f,
                longitude: points[i].longitude
                    + (points[i + 1].longitude - points[i].longitude) * f,
            };
        }
    }
    *points.last().expect("validated source points")
}
fn slice_matches(slice: &ProvenanceGeometrySlice, current: &[GeoPoint]) -> bool {
    let (source_positions, source_length) = geometry_positions(&slice.original_points);
    let (current_positions, current_length) = geometry_positions(current);
    if source_length <= 0. || current_length <= 0. {
        return false;
    }
    let width = slice.to_position - slice.from_position;
    // Local planar geodesic approximation introduces tiny subdivision error.
    let tolerance = 0.01 + source_length * 1e-6;
    if (current_length - source_length * width).abs() > tolerance {
        return false;
    }
    for (point, t) in current.iter().zip(&current_positions) {
        let expected = geometry_point(
            &slice.original_points,
            &source_positions,
            slice.from_position + width * t,
        );
        if super::alignment::distance(*point, expected) > tolerance {
            return false;
        }
    }
    // Also check source vertices: endpoint-only matching cannot validate bends.
    for (point, t) in slice.original_points.iter().zip(&source_positions) {
        if *t > slice.from_position && *t < slice.to_position {
            let actual = geometry_point(
                current,
                &current_positions,
                (t - slice.from_position) / width,
            );
            if super::alignment::distance(*point, actual) > tolerance {
                return false;
            }
        }
    }
    true
}
fn safe_text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len()<=max && !s.chars().any(|c|c.is_control() || matches!(c as u32,0x00ad|0x061c|0x180e|0x200b..=0x200f|0x202a..=0x202e|0x2060..=0x206f|0xfeff|0xfff9..=0xfffb|0xe0001|0xe0020..=0xe007f))
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl SourceRecord {
    pub(super) fn valid(&self) -> bool {
        hash(&self.graph_sha256)
            && hash(&self.source_sha256)
            && self.key == format!("{}:{}", self.graph_sha256, self.source_sha256)
            && safe_text(&self.dataset_id, 200)
            && safe_text(&self.revision, 200)
            && safe_text(&self.source, 1000)
            && self.source_date.as_ref().is_none_or(|s| safe_text(s, 64))
            && self.license == "ODbL-1.0"
            && self.attribution == "© OpenStreetMap contributors"
            && matches!(self.conversion_format_version, 1 | 2)
    }
    fn bytes(&self) -> usize {
        self.key.len()
            + self.dataset_id.len()
            + self.revision.len()
            + self.graph_sha256.len()
            + self.source_sha256.len()
            + self.source.len()
            + self.source_date.as_ref().map_or(0, |s| s.len())
            + self.license.len()
            + self.attribution.len()
            + 256
    }
}
impl AlignmentProvenanceInput {
    pub(super) fn matches_source(&self, source: &SourceRecord) -> bool {
        self.source_key == source.key
            && source.conversion_format_version
                == if self.method == ProvenanceMethod::RailGraphRouteV1 {
                    2
                } else {
                    1
                }
    }

    pub(super) fn valid(&self) -> bool {
        if self.method == ProvenanceMethod::RailGraphRouteV1 {
            return self.osm_way_id.is_empty()
                && self.mode == ReferenceMode::Rail
                && self.start_segment_index == 0
                && self.end_segment_index == 0
                && self.start_fraction == 0.0
                && self.end_fraction == 0.0
                && (1..=256).contains(&self.network_path.len())
                && self.network_path.iter().all(|span| {
                    span.region == "jp"
                        && span.chain_id < 600_000
                        && span.from_fraction.is_finite()
                        && span.to_fraction.is_finite()
                        && (0.0..=1.0).contains(&span.from_fraction)
                        && (0.0..=1.0).contains(&span.to_fraction)
                        && span.from_fraction != span.to_fraction
                });
        }
        if !self.network_path.is_empty() {
            return false;
        }
        let id = &self.osm_way_id;
        !id.is_empty()
            && id.len() <= 20
            && id.as_bytes()[0] != b'0'
            && id.bytes().all(|b| b.is_ascii_digit())
            && self.start_segment_index <= 9_007_199_254_740_991
            && self.end_segment_index <= 9_007_199_254_740_991
            && self.start_fraction.is_finite()
            && (0.0..=1.0).contains(&self.start_fraction)
            && self.end_fraction.is_finite()
            && (0.0..=1.0).contains(&self.end_fraction)
            && (self.start_segment_index != self.end_segment_index
                || self.start_fraction != self.end_fraction)
            && !(self.start_segment_index.checked_add(1) == Some(self.end_segment_index)
                && self.start_fraction == 1.0
                && self.end_fraction == 0.0)
            && !(self.end_segment_index.checked_add(1) == Some(self.start_segment_index)
                && self.end_fraction == 1.0
                && self.start_fraction == 0.0)
            && (self.method != ProvenanceMethod::OsmSegmentCopyV1
                || self.start_segment_index == self.end_segment_index)
    }
}
impl WorldDraft {
    pub(super) fn validate_provenance(&self) -> Result<(), WorldError> {
        if self.source_records.len() > MAX_SOURCES
            || self.alignment_provenance.len() > MAX_PROVENANCE
        {
            return Err(WorldError::InvalidProvenance);
        }
        let mut keys = BTreeSet::new();
        let mut bytes = 0;
        for source in &self.source_records {
            if !source.valid() || !keys.insert(&source.key) {
                return Err(WorldError::InvalidProvenance);
            }
            bytes += source.bytes();
        }
        let mut spans = std::collections::BTreeMap::<AlignmentId, Vec<(usize, usize)>>::new();
        for p in &self.alignment_provenance {
            if !self.alignments.iter().any(|a| {
                a.id == p.alignment_id
                    && p.start_point_index < p.end_point_index
                    && p.end_point_index < a.points.len()
            }) || !keys.contains(&p.source.source_key)
                || !p.source.valid()
                || !self
                    .source_records
                    .iter()
                    .any(|record| p.source.matches_source(record))
            {
                return Err(WorldError::InvalidProvenance);
            }
            if let Some(slice) = &p.derived_slice {
                if !(2..=4096).contains(&slice.original_points.len())
                    || !slice.from_position.is_finite()
                    || !slice.to_position.is_finite()
                    || slice.from_position < 0.0
                    || slice.to_position > 1.0
                    || slice.from_position >= slice.to_position
                    || slice.original_points.iter().any(|point| {
                        !point.latitude.is_finite()
                            || !point.longitude.is_finite()
                            || point.latitude.abs() > 85.0
                            || point.longitude.abs() > 180.0
                    })
                {
                    return Err(WorldError::InvalidProvenance);
                }
                let alignment = self
                    .alignments
                    .iter()
                    .find(|a| a.id == p.alignment_id)
                    .expect("validated source alignment");
                if !slice_matches(
                    slice,
                    &alignment.points[p.start_point_index..=p.end_point_index],
                ) {
                    return Err(WorldError::InvalidProvenance);
                }
                bytes += slice.original_points.len() * 16 + 32;
            }
            let ranges = spans.entry(p.alignment_id).or_default();
            if ranges
                .iter()
                .any(|(start, end)| p.start_point_index < *end && *start < p.end_point_index)
            {
                return Err(WorldError::InvalidProvenance);
            }
            ranges.push((p.start_point_index, p.end_point_index));
            bytes += p.source.source_key.len()
                + p.source.osm_way_id.len()
                + 256
                + p.source.network_path.len() * 64;
        }
        if bytes > MAX_METADATA_BYTES {
            return Err(WorldError::InvalidProvenance);
        }
        Ok(())
    }
    /// Only endpoint growth may move a source span. Restore the old geometry and
    /// indexes before invoking the unchanged generic provenance protection.
    pub(super) fn preserve_extended_provenance(
        &self,
        old: &Self,
        id: AlignmentId,
        prepend: usize,
    ) -> Result<(), WorldError> {
        let mut normalized = self.clone();
        let previous = old
            .alignments
            .iter()
            .find(|a| a.id == id)
            .ok_or(WorldError::InvalidAlignment)?;
        let current = normalized
            .alignments
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(WorldError::InvalidAlignment)?;
        let end = prepend
            .checked_add(previous.points.len())
            .ok_or(WorldError::InvalidProvenance)?;
        if current.points.get(prepend..end) != Some(previous.points.as_slice()) {
            return Err(WorldError::ProvenanceEditUnsupported);
        }
        current.points = previous.points.clone();
        current.anchor_indices = previous.anchor_indices.clone();
        for p in normalized
            .alignment_provenance
            .iter_mut()
            .filter(|p| p.alignment_id == id)
        {
            p.start_point_index = p
                .start_point_index
                .checked_sub(prepend)
                .ok_or(WorldError::InvalidProvenance)?;
            p.end_point_index = p
                .end_point_index
                .checked_sub(prepend)
                .ok_or(WorldError::InvalidProvenance)?;
        }
        normalized.preserve_provenance(old)
    }
    pub(super) fn preserve_provenance(&self, old: &Self) -> Result<(), WorldError> {
        // Source snapshots survive removing a bundle and are not silently GC'd.
        for source in &old.source_records {
            if !self.source_records.contains(source) {
                return Err(WorldError::ProvenanceEditUnsupported);
            }
        }
        for p in &old.alignment_provenance {
            if let Some(alignment) = self.alignments.iter().find(|a| a.id == p.alignment_id) {
                if !self.alignment_provenance.contains(p)
                    || old
                        .alignments
                        .iter()
                        .find(|a| a.id == p.alignment_id)
                        .map(|a| &a.points)
                        != Some(&alignment.points)
                {
                    return Err(WorldError::ProvenanceEditUnsupported);
                }
            }
        }
        // Provenance cannot be retroactively attached using generic replace.
        if self
            .alignment_provenance
            .iter()
            .any(|p| !old.alignment_provenance.contains(p))
            || self
                .source_records
                .iter()
                .any(|s| !old.source_records.contains(s))
        {
            return Err(WorldError::ProvenanceEditUnsupported);
        }
        Ok(())
    }
}
impl World {
    /// Copy geometry plus its claimed source snapshot atomically. The caller must
    /// verify the bundle; hashes do not prove that supplied points came from it.
    pub fn build_alignment_from_reference(
        &mut self,
        owner: CompanyId,
        name: String,
        points: Vec<GeoPoint>,
        source_record: SourceRecord,
        provenance: AlignmentProvenanceInput,
    ) -> Result<AlignmentId, WorldError> {
        self.build_alignment_from_reference_typed(
            owner,
            name,
            points,
            source_record,
            provenance,
            TrackType::Normal,
        )
    }
    pub fn build_alignment_from_reference_typed(
        &mut self,
        owner: CompanyId,
        name: String,
        points: Vec<GeoPoint>,
        source_record: SourceRecord,
        provenance: AlignmentProvenanceInput,
        track_type: TrackType,
    ) -> Result<AlignmentId, WorldError> {
        if provenance.source_key != source_record.key {
            return Err(WorldError::InvalidProvenance);
        }
        if let Some(existing) = self
            .data
            .source_records
            .iter()
            .find(|s| s.key == source_record.key)
        {
            if existing != &source_record {
                return Err(WorldError::InvalidProvenance);
            }
        }
        self.insert(|data, id| {
            if !data
                .source_records
                .iter()
                .any(|s| s.key == source_record.key)
            {
                data.source_records.push(source_record);
            }
            let end_point_index = points.len().saturating_sub(1);
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
            });
            data.alignment_provenance.push(AlignmentProvenance {
                derived_slice: None,
                alignment_id: AlignmentId(id),
                source: provenance,
                start_point_index: 0,
                end_point_index,
            });
        })
        .map(AlignmentId)
    }
}

impl World {
    /// Extend along a verified reference slice while retaining every older source
    /// range and license snapshot. Input points run from the old endpoint outward.
    pub fn extend_alignment_from_reference(
        &mut self,
        owner: CompanyId,
        id: AlignmentId,
        endpoint: AlignmentEndpoint,
        points: Vec<GeoPoint>,
        source_record: SourceRecord,
        mut source: AlignmentProvenanceInput,
    ) -> Result<(), WorldError> {
        if source.source_key != source_record.key
            || self
                .data
                .source_records
                .iter()
                .any(|s| s.key == source_record.key && s != &source_record)
        {
            return Err(WorldError::InvalidProvenance);
        }
        let old_count = self
            .data
            .alignments
            .iter()
            .find(|a| a.id == id)
            .ok_or(WorldError::MissingReference("alignment"))?
            .points
            .len();
        let added_count = points
            .len()
            .checked_sub(1)
            .ok_or(WorldError::InvalidAlignment)?;
        let mut candidate = self.clone();
        candidate.extend_alignment(owner, id, endpoint, points)?;
        let (start_point_index, end_point_index) = match endpoint {
            AlignmentEndpoint::End => (old_count - 1, old_count - 1 + added_count),
            AlignmentEndpoint::Start => {
                std::mem::swap(
                    &mut source.start_segment_index,
                    &mut source.end_segment_index,
                );
                std::mem::swap(&mut source.start_fraction, &mut source.end_fraction);
                source.network_path.reverse();
                for span in &mut source.network_path {
                    std::mem::swap(&mut span.from_fraction, &mut span.to_fraction);
                }
                (0, added_count)
            }
        };
        if !candidate
            .data
            .source_records
            .iter()
            .any(|s| s.key == source_record.key)
        {
            candidate.data.source_records.push(source_record);
        }
        candidate
            .data
            .alignment_provenance
            .push(AlignmentProvenance {
                derived_slice: None,
                alignment_id: id,
                source,
                start_point_index,
                end_point_index,
            });
        candidate.data.validate()?;
        *self = candidate;
        Ok(())
    }
}
