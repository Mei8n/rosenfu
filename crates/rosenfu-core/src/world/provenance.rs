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
    pub source_key: String,
    pub method: ProvenanceMethod,
    pub osm_way_id: String,
    pub mode: ReferenceMode,
    pub start_segment_index: u64,
    pub start_fraction: f64,
    pub end_segment_index: u64,
    pub end_fraction: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentProvenance {
    pub alignment_id: AlignmentId,
    pub source: AlignmentProvenanceInput,
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
    fn valid(&self) -> bool {
        hash(&self.graph_sha256)
            && hash(&self.source_sha256)
            && self.key == format!("{}:{}", self.graph_sha256, self.source_sha256)
            && safe_text(&self.dataset_id, 200)
            && safe_text(&self.revision, 200)
            && safe_text(&self.source, 1000)
            && self.source_date.as_ref().is_none_or(|s| safe_text(s, 64))
            && self.license == "ODbL-1.0"
            && self.attribution == "© OpenStreetMap contributors"
            && self.conversion_format_version == 1
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
    fn valid(&self) -> bool {
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
        let mut ids = BTreeSet::new();
        for p in &self.alignment_provenance {
            if !ids.insert(p.alignment_id)
                || !self.alignments.iter().any(|a| a.id == p.alignment_id)
                || !keys.contains(&p.source.source_key)
                || !p.source.valid()
            {
                return Err(WorldError::InvalidProvenance);
            }
            bytes += p.source.source_key.len() + p.source.osm_way_id.len() + 256;
        }
        if bytes > MAX_METADATA_BYTES {
            return Err(WorldError::InvalidProvenance);
        }
        Ok(())
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
            data.alignments.push(Alignment {
                id: AlignmentId(id),
                owner,
                name,
                points,
                track_type,
            });
            data.alignment_provenance.push(AlignmentProvenance {
                alignment_id: AlignmentId(id),
                source: provenance,
            });
        })
        .map(AlignmentId)
    }
}
