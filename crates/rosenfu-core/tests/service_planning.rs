// The deleted track-only preview rejected alignments. These tests now exercise
// the supported explicit path preview; route slicing is covered in session tests.
mod common;
use common::*;
use rosenfu_core::world::*;
#[test]
fn forward_reverse_partial_and_repeated_occurrences_are_explicit() {
    let (w, _, a, s) = line(3);
    let before = w.clone();
    for (start, segs, expected) in [
        (s[0], vec![segment(a, s[0], s[2])], s.clone()),
        (s[2], vec![segment(a, s[2], s[0])], vec![s[2], s[1], s[0]]),
        (s[1], vec![segment(a, s[1], s[2])], vec![s[1], s[2]]),
        (
            s[0],
            vec![segment(a, s[0], s[2]), segment(a, s[2], s[0])],
            vec![s[0], s[1], s[2], s[1], s[0]],
        ),
    ] {
        let p = w.preview_service_path(start, segs).unwrap();
        assert_eq!(
            p.occurrences
                .iter()
                .map(|o| o.station_id)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(p.requires_stop_selection);
        assert_eq!(p.occurrences[0].reference, StationOccurrenceRef::Start);
    }
    assert_eq!(w, before);
}
#[test]
fn exclusions_are_membership_not_stop_decisions() {
    let (mut w, c, a, s) = line(3);
    let seg = vec![segment(a, s[0], s[2])];
    w.add_planned_route(c, "R".into(), s[0], seg.clone(), vec![s[0], s[2]])
        .unwrap();
    let before = w.clone();
    let p = w.preview_service_path(s[0], seg).unwrap();
    assert_eq!(
        p.occurrences
            .iter()
            .map(|o| o.station_id)
            .collect::<Vec<_>>(),
        s
    );
    assert!(p.requires_stop_selection);
    assert!(serde_json::to_value(p).unwrap().get("stops").is_none());
    assert_eq!(w, before);
}
#[test]
fn invalid_inputs_are_read_only() {
    let (w, _, a, s) = line(3);
    let before = w.clone();
    for (start, segs) in [
        (StationId(999), vec![segment(a, s[0], s[2])]),
        (s[0], vec![]),
        (s[0], vec![segment(a, s[0], s[0])]),
        (s[0], vec![segment(AlignmentId(999), s[0], s[2])]),
        (s[0], vec![segment(a, s[1], s[2])]),
    ] {
        assert!(w.preview_service_path(start, segs).is_err());
        assert_eq!(w, before);
    }
}
#[test]
fn alignment_sections_are_supported_without_silent_stop_selection() {
    let (w, _, a, s) = line(5);
    let p = w
        .preview_service_path(s[1], vec![segment(a, s[1], s[4])])
        .unwrap();
    assert_eq!(
        p.occurrences
            .iter()
            .map(|o| o.station_id)
            .collect::<Vec<_>>(),
        s[1..]
    );
    assert!(p.requires_stop_selection);
    assert_eq!(p.legs.len(), 1);
}
#[test]
fn multiple_legs_keep_occurrence_references_after_reverse() {
    let (w, _, a, s) = line(5);
    let p = w
        .preview_service_path(s[4], vec![segment(a, s[4], s[2]), segment(a, s[2], s[0])])
        .unwrap();
    assert_eq!(
        p.occurrences
            .iter()
            .map(|o| o.station_id)
            .collect::<Vec<_>>(),
        s.iter().rev().copied().collect::<Vec<_>>()
    );
    assert_eq!(
        p.occurrences[2].reference,
        StationOccurrenceRef::OnLeg {
            leg_key: 1,
            station_id: s[2]
        }
    );
    assert_eq!(
        p.occurrences[3].reference,
        StationOccurrenceRef::OnLeg {
            leg_key: 2,
            station_id: s[1]
        }
    );
}
