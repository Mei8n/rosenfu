//! Experimental sufficient condition, not a construction authorization API.
//! The caller must provide EVERY span of the affected connected component after
//! editing, including existing edges, each once (edge-simple route semantics).
//! Heights/tangents are exact micrometres, not rounded samples from an engine.
//! Lengths are conservative integer millimetre upper bounds in the engine's
//! distance metric. No adapter from Rosenfu's f64 profiles is implemented here.
//! Unknown water is assumed possible everywhere; no dry coverage is inferred.

#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub start_height_um: i64,
    pub end_height_um: i64,
    /// Derivative with respect to normalized span parameter t, not metres.
    pub start_tangent_um: i64,
    pub end_tangent_um: i64,
    pub length_upper_mm: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Inconclusive {
    EmptyComponent,
    InvalidLength,
    PossibleGroundContact,
    LengthOverflow,
    UpperBoundExceedsLimit,
}
/// A conditional proof only: no data-completeness or road-safety claim.
#[derive(Debug, PartialEq, Eq)]
pub struct BoundProof {
    pub constrained_length_upper_mm: u64,
}

pub fn prove_supplied_component(spans: &[Span]) -> Result<BoundProof, Inconclusive> {
    if spans.is_empty() {
        return Err(Inconclusive::EmptyComponent);
    }
    let mut total = 0u64;
    for s in spans {
        if s.length_upper_mm == 0 {
            return Err(Inconclusive::InvalidLength);
        }
        // Three times the Bezier control heights for the exact Hermite cubic.
        // i128 prevents overflow even for all extreme i64 inputs. A cubic stays
        // inside its control hull; rejection of a loose hull is conservative.
        let a = i128::from(s.start_height_um);
        let b = i128::from(s.end_height_um);
        let controls = [
            3 * a,
            3 * a + i128::from(s.start_tangent_um),
            3 * b - i128::from(s.end_tangent_um),
            3 * b,
        ];
        // Exclude the core flat-ground tolerance band +/-1 micrometre too.
        // Applying that band to all curves is conservative.
        if !(controls.iter().all(|v| *v > 3) || controls.iter().all(|v| *v < -3)) {
            return Err(Inconclusive::PossibleGroundContact);
        }
        // The ENTIRE positive-length span must be at/below -12 m. Touching that
        // depth once cannot reset a continuous run. We never reset runs here:
        // sum all non-exempt spans, even across dry/immune gaps and branches.
        if !controls.iter().all(|v| *v <= -36_000_000) {
            total = total
                .checked_add(s.length_upper_mm)
                .ok_or(Inconclusive::LengthOverflow)?;
        }
    }
    if total > 30_000_000 {
        return Err(Inconclusive::UpperBoundExceedsLimit);
    }
    Ok(BoundProof {
        constrained_length_upper_mm: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn flat(h: i64, mm: u64) -> Span {
        Span {
            start_height_um: h,
            end_height_um: h,
            start_tangent_um: 0,
            end_tangent_um: 0,
            length_upper_mm: mm,
        }
    }
    #[test]
    fn thirty_km_is_allowed_but_one_mm_more_is_not() {
        assert_eq!(
            prove_supplied_component(&[flat(6_000_000, 30_000_000)])
                .unwrap()
                .constrained_length_upper_mm,
            30_000_000
        );
        assert_eq!(
            prove_supplied_component(&[flat(6_000_000, 30_000_001)]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn ground_and_zero_endpoint_are_not_proven() {
        assert_eq!(
            prove_supplied_component(&[flat(0, 1)]),
            Err(Inconclusive::PossibleGroundContact)
        );
        let s = Span {
            end_height_um: 0,
            ..flat(6_000_000, 1)
        };
        assert_eq!(
            prove_supplied_component(&[s]),
            Err(Inconclusive::PossibleGroundContact)
        );
    }
    #[test]
    fn positive_endpoints_do_not_hide_interior_zero() {
        // h(t)=1-8t+8t^2, in metres: both endpoints positive, centre negative.
        let s = Span {
            start_tangent_um: -8_000_000,
            end_tangent_um: 8_000_000,
            ..flat(1_000_000, 1)
        };
        assert_eq!(
            prove_supplied_component(&[s]),
            Err(Inconclusive::PossibleGroundContact)
        );
    }
    #[test]
    fn tangent_contact_with_zero_is_not_proven() {
        // h(t)=(2t-1)^2 metres, zero at t=.5 without a sign change.
        let s = Span {
            start_tangent_um: -4_000_000,
            end_tangent_um: 4_000_000,
            ..flat(1_000_000, 1)
        };
        assert_eq!(
            prove_supplied_component(&[s]),
            Err(Inconclusive::PossibleGroundContact)
        );
    }
    #[test]
    fn deep_tunnel_exemption_and_cutting() {
        for h in [-12_000_000, -18_000_000] {
            assert_eq!(
                prove_supplied_component(&[flat(h, u64::MAX)])
                    .unwrap()
                    .constrained_length_upper_mm,
                0
            );
        }
        assert_eq!(
            prove_supplied_component(&[flat(-6_000_000, 30_000_001)]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn deep_endpoints_do_not_hide_shallower_middle() {
        let s = Span {
            start_tangent_um: 12_000_000,
            end_tangent_um: -12_000_000,
            ..flat(-12_000_000, 30_000_001)
        };
        assert_eq!(
            prove_supplied_component(&[s]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn touching_minus_twelve_once_does_not_reset_length() {
        // h(t)=-11-4t+4t^2, touches -12 only at centre.
        let s = Span {
            start_tangent_um: -4_000_000,
            end_tangent_um: 4_000_000,
            ..flat(-11_000_000, 30_000_001)
        };
        assert_eq!(
            prove_supplied_component(&[s]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn connected_old_edges_and_branches_must_be_included() {
        assert!(prove_supplied_component(&[flat(6_000_000, 20_000_000)]).is_ok());
        assert_eq!(
            prove_supplied_component(&[flat(6_000_000, 20_000_000), flat(6_000_000, 20_000_000)]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn ring_seam_cannot_hide_total() {
        assert_eq!(
            prove_supplied_component(&[flat(6_000_000, 15_000_000), flat(6_000_000, 15_000_001)]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn invalid_and_overflow_inputs_fail_closed() {
        assert_eq!(
            prove_supplied_component(&[]),
            Err(Inconclusive::EmptyComponent)
        );
        assert_eq!(
            prove_supplied_component(&[flat(1, 0)]),
            Err(Inconclusive::InvalidLength)
        );
        assert_eq!(
            prove_supplied_component(&[flat(2, u64::MAX), flat(2, 1)]),
            Err(Inconclusive::LengthOverflow)
        );
        assert!(prove_supplied_component(&[flat(i64::MIN, 1)]).is_ok());
        assert!(prove_supplied_component(&[flat(i64::MAX, 1)]).is_ok());
    }
    #[test]
    fn core_ground_tolerance_is_excluded() {
        for h in [-1, 0, 1] {
            assert_eq!(
                prove_supplied_component(&[flat(h, 1)]),
                Err(Inconclusive::PossibleGroundContact)
            );
        }
        for h in [-2, 2] {
            assert!(prove_supplied_component(&[flat(h, 1)]).is_ok());
        }
    }
    #[test]
    fn safe_curved_profile_can_be_proven() {
        let s = Span {
            start_tangent_um: 3_000_000,
            end_tangent_um: -3_000_000,
            ..flat(6_000_000, 1)
        };
        assert!(prove_supplied_component(&[s]).is_ok());
    }
    #[test]
    fn extreme_heights_and_tangents_do_not_panic() {
        for a in [i64::MIN, i64::MAX] {
            for b in [i64::MIN, i64::MAX] {
                for c in [i64::MIN, i64::MAX] {
                    for d in [i64::MIN, i64::MAX] {
                        let _ = prove_supplied_component(&[Span {
                            start_height_um: a,
                            end_height_um: b,
                            start_tangent_um: c,
                            end_tangent_um: d,
                            length_upper_mm: 1,
                        }]);
                    }
                }
            }
        }
    }
    #[test]
    fn immune_gap_does_not_reduce_the_component_sum() {
        assert_eq!(
            prove_supplied_component(&[
                flat(6_000_000, 20_000_000),
                flat(-12_000_000, 1),
                flat(6_000_000, 20_000_000)
            ]),
            Err(Inconclusive::UpperBoundExceedsLimit)
        );
    }
    #[test]
    fn every_wet_assignment_and_edge_order_is_bounded() {
        // Exhaust wet/dry choices and permutations: any edge-simple traversal
        // through any topology is a subset/order of the component's edges.
        let lengths = [2_000_000u64, 7_000_000, 8_000_000, 13_000_000];
        fn permutations(v: &mut [usize], at: usize, visit: &mut impl FnMut(&[usize])) {
            if at == v.len() {
                visit(v);
                return;
            }
            for j in at..v.len() {
                v.swap(at, j);
                permutations(v, at + 1, visit);
                v.swap(at, j);
            }
        }
        let proof = prove_supplied_component(&lengths.map(|n| flat(6_000_000, n))).unwrap();
        for wet in 0..16 {
            for exempt in 0..16 {
                permutations(&mut [0, 1, 2, 3], 0, &mut |order| {
                    let mut run = 0;
                    let mut longest = 0;
                    for &i in order {
                        if wet & (1 << i) != 0 && exempt & (1 << i) == 0 {
                            run += lengths[i];
                        } else {
                            run = 0;
                        }
                        longest = longest.max(run);
                    }
                    assert!(longest <= proof.constrained_length_upper_mm);
                });
            }
        }
    }
}
