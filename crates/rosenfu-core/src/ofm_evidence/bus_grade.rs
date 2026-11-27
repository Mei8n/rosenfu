//! Exact original-road topology used only by opted-in ordinary Bus proofs.
use super::{number, RawValue, TileLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Grade {
    Ground,
    Layer(i8),
    Bridge(i8),
    Tunnel(i8),
}

impl Grade {
    pub(super) fn node_suffix(self) -> String {
        match self {
            Self::Ground => String::new(),
            Self::Layer(n) => format!(":layer:{n}"),
            Self::Bridge(n) => format!(":bridge:{n}"),
            Self::Tunnel(n) => format!(":tunnel:{n}"),
        }
    }
    pub(super) fn is_ground(self) -> bool {
        self == Self::Ground
    }
}

fn str_tag<'a>(line: &'a TileLine, key: &str) -> Option<&'a str> {
    match line.attributes.get(key) {
        Some(RawValue::String(v)) => Some(v),
        _ => None,
    }
}

pub(super) fn road_class(line: &TileLine) -> Option<&str> {
    str_tag(line, "class")
}
pub(super) fn supported_class(line: &TileLine) -> bool {
    matches!(
        road_class(line),
        Some(
            "motorway"
                | "trunk"
                | "primary"
                | "secondary"
                | "tertiary"
                | "minor"
                | "service"
                | "busway"
                | "bus_guideway"
        )
    )
}
pub(super) fn fast_class(line: &TileLine) -> bool {
    matches!(road_class(line), Some("motorway" | "trunk"))
}

pub(super) fn grade(line: &TileLine) -> Option<Grade> {
    let layer = match line.attributes.get("layer") {
        None => 0.,
        Some(RawValue::String(s)) => s.parse::<f64>().ok()?,
        Some(v) => number(v)?,
    };
    if !(-8.0..=8.0).contains(&layer) || layer.fract() != 0. {
        return None;
    }
    let layer = layer as i8;
    match str_tag(line, "brunnel") {
        Some("bridge") => Some(Grade::Bridge(layer)),
        Some("tunnel") => Some(Grade::Tunnel(layer)),
        None => Some(if layer == 0 {
            Grade::Ground
        } else {
            Grade::Layer(layer)
        }),
        _ => None,
    }
}

pub(super) fn one_way(line: &TileLine) -> Option<i8> {
    match line.attributes.get("oneway") {
        None => Some(0),
        Some(RawValue::String(s)) => match s.as_str() {
            "no" | "0" => Some(0),
            "yes" | "1" => Some(1),
            "-1" => Some(-1),
            _ => None,
        },
        Some(v) => match number(v)? {
            0. => Some(0),
            1. => Some(1),
            -1. => Some(-1),
            _ => None,
        },
    }
}

pub(super) fn allowed_access(line: &TileLine) -> bool {
    match line.attributes.get("access") {
        None => true, // ordinary simulated eligibility, not a real traffic claim
        Some(RawValue::String(s)) => matches!(s.as_str(), "yes" | "permissive" | "designated"),
        _ => false,
    }
}

/// Exactly observed two-end continuation; no snap/slope/junction creation.
/// Matching class/subclass and opposed local direction reduce ambiguity when
/// source features have independent tile ordinals rather than OSM way IDs.
pub(super) fn exact_continuation(
    a: &TileLine,
    a_vertex: usize,
    a_grade: Grade,
    b: &TileLine,
    b_vertex: usize,
    b_grade: Grade,
) -> bool {
    let valid_grade = a_grade == b_grade
        || matches!(
            (a_grade, b_grade),
            (Grade::Ground, Grade::Bridge(_) | Grade::Tunnel(_))
                | (Grade::Bridge(_) | Grade::Tunnel(_), Grade::Ground)
        );
    if !valid_grade
        || road_class(a) != road_class(b)
        || str_tag(a, "subclass") != str_tag(b, "subclass")
    {
        return false;
    }
    let approach = |line: &TileLine, idx: usize| {
        let p = line.coordinates[idx];
        let q = if idx == 0 {
            line.coordinates[1]
        } else if idx + 1 == line.coordinates.len() {
            line.coordinates[idx - 1]
        } else {
            return None;
        };
        Some([
            i128::from(q[0]) - i128::from(p[0]),
            i128::from(q[1]) - i128::from(p[1]),
        ])
    };
    let (Some(x), Some(y)) = (approach(a, a_vertex), approach(b, b_vertex)) else {
        return false;
    };
    let dot = x[0] * y[0] + x[1] * y[1];
    let square = |v: [i128; 2]| v[0] * v[0] + v[1] * v[1];
    // Opposed approach vectors within 30° of a straight continuation.
    dot < 0 && 4 * dot * dot >= 3 * square(x) * square(y)
}
