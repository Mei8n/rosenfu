use super::*;
fn service(id: &str, resource: &str) -> ServiceRunSpec {
    ServiceRunSpec {
        service_id: id.into(),
        vehicle: Vehicle {
            max_speed_kmh: 72.0,
            acceleration_mps2: 1.0,
            braking_mps2: 1.0,
            length_m: 20.0,
        },
        timetable: Timetable {
            first_departure_ms: 0,
            last_departure_ms: 0,
            headway_ms: 60000,
        },
        legs: vec![RunLeg {
            from: "1".into(),
            to: "2".into(),
            dwell_ms: 10000,
            sections: vec![Section {
                length_m: 100.0,
                speed_limit_kmh: 72.0,
                resources: vec![resource.into()],
            }],
        }],
    }
}
#[test]
fn triangular_short_run() {
    let s = service("1", "a");
    assert_eq!(running_time_ms(&s.legs[0].sections, &s.vehicle), Ok(20000));
}
#[test]
fn lower_cap_increases_time() {
    let s = service("1", "a");
    let mut slow = s.clone();
    slow.legs[0].sections[0].speed_limit_kmh = 18.;
    assert!(running_time_ms(&slow.legs[0].sections, &slow.vehicle).unwrap() > 20000);
}
#[test]
fn conflicting_trains_wait_but_independent_run() {
    let mut r = OperationRuntime::new(vec![
        service("1", "a"),
        service("2", "a"),
        service("3", "b"),
    ])
    .unwrap();
    r.advance_to(0, 10).unwrap();
    let t: Vec<_> = r.trains().collect();
    assert!(matches!(t[0].state, TrainState::Moving { .. }));
    assert!(matches!(t[1].state, TrainState::Waiting { .. }));
    assert!(matches!(t[2].state, TrainState::Moving { .. }));
    let out = r.advance_to(50000, 100).unwrap();
    assert_eq!(out.arrivals.len(), 3);
    assert!(out
        .arrivals
        .iter()
        .any(|a| a.train.service_id == "2" && a.delay_ms > 0));
    assert_eq!(r.trains().count(), 0);
    assert!(r.occupied.is_empty());
}
#[test]
fn frame_chunking_and_save_replay_are_deterministic() {
    let mut s = service("1", "a");
    s.timetable.last_departure_ms = 120000;
    let mut one = OperationRuntime::new(vec![s]).unwrap();
    let mut many = one.clone();
    let expected = one.advance_to(200000, 100).unwrap().arrivals;
    let mut actual = Vec::new();
    for now in (0..=200000).step_by(1000) {
        actual.extend(many.advance_to(now, 100).unwrap().arrivals);
    }
    assert_eq!(expected, actual);
    assert_eq!(one.save(), many.save());
    let restored = OperationRuntime::restore(many.save()).unwrap();
    assert_eq!(restored.trains, one.trains);
}
#[test]
fn pause_speed_and_backward_time() {
    let mut r = OperationRuntime::new(vec![service("1", "a")]).unwrap();
    assert_eq!(r.wall_target(1000), Ok(0));
    r.set_paused(false);
    r.set_speed(4).unwrap();
    assert_eq!(r.wall_target(1000), Ok(4000));
    r.advance_to(4000, 100).unwrap();
    assert_eq!(r.advance_to(3999, 100), Err(OperationError::InvalidClock));
    assert_eq!(r.set_speed(0), Err(OperationError::InvalidClock));
}
#[test]
fn budget_is_backpressure_not_dropped_trains() {
    let mut s = service("1", "a");
    s.timetable.last_departure_ms = 200000;
    let mut r = OperationRuntime::new(vec![s]).unwrap();
    let mut baseline = r.clone();
    let expected = baseline.advance_to(300000, 1000).unwrap().arrivals;
    let mut actual = Vec::new();
    loop {
        let result = r.advance_to(300000, 1).unwrap();
        actual.extend(result.arrivals);
        if result.reached_target {
            break;
        }
    }
    assert_eq!(actual, expected);
}
#[test]
fn invalid_input_rejected() {
    let mut s = service("1", "a");
    s.timetable.headway_ms = 0;
    assert!(OperationRuntime::new(vec![s]).is_err());
    let mut s = service("1", "a");
    s.vehicle.acceleration_mps2 = f64::NAN;
    assert!(OperationRuntime::new(vec![s]).is_err());
}
#[test]
fn intermediate_stop_affects_passenger_time() {
    let mut s = service("1", "a");
    let mut l = s.legs[0].clone();
    l.from = "2".into();
    l.to = "3".into();
    s.legs.push(l);
    let r = OperationRuntime::new(vec![s]).unwrap();
    let offers = r.passenger_offers();
    assert_eq!(offers[0].ride_seconds, 20.);
    assert_eq!(offers[1].ride_seconds, 30.);
}
