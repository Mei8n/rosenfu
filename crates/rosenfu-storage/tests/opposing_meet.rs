use rosenfu_core::operation::{Timetable, TrainState, Vehicle};
use rosenfu_core::world::{
    AlignmentConnection,
    AlignmentEndpoint::{End, Start},
    AlignmentEndpointRef, AlignmentId, GeoPoint, OperationServiceConfig, RouteSegment,
    SectionInfrastructure, StationId, World,
};

#[path = "../../rosenfu-core/tests/fixtures/opposing_meet_fixture.rs"]
mod fixture;
use fixture::meeting_loop;

// Ordinary format 2 intentionally keeps the far throat reserved through rear
// clearance. A physical second bay alone does not enable an opposing meet.
#[test]
fn ordinary_single_track_serializes_opposing_arrivals_even_with_distinct_bays() {
    for separate in [false, true] {
        let (mut world, configs, [west, east, main, side], [a, b, s, c]) = meeting_loop(separate);
        world.configure_operation(configs.to_vec()).unwrap();
        let ids = configs.map(|x| x.service_id.0.to_string());
        let rt = &mut world.operation_mut().unwrap().runtime;
        let specs = ids.each_ref().map(|id| rt.service_spec(id).unwrap());
        assert_eq!(
            specs.map(|s| s.legs.len()),
            [2, 2],
            "auto reversal would confound a two-movement test"
        );
        assert_eq!(rt.save().format, 2);
        let keys = |r: usize| {
            specs[r].legs[0]
                .sections
                .iter()
                .flat_map(|s| s.resources.iter().cloned())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let east_in = keys(0);
        let west_in = keys(1);
        let stem = |id: AlignmentId, end: &str| format!("rail:join:{}:{end}", id.0);
        assert!(east_in.contains(&stem(west, "End")));
        assert!(west_in.contains(&stem(east, "Start")));
        assert!(
            east_in.contains(&stem(east, "Start")),
            "outgoing throat added before B"
        );
        assert!(
            west_in.contains(&stem(west, "End")),
            "outgoing throat added before B"
        );
        let lane = |id: AlignmentId| format!("rail:{}:section:0:lane:0", id.0);
        assert!(east_in.contains(&lane(main)));
        assert!(west_in.contains(&lane(if separate { side } else { main })));
        assert_eq!(west_in.contains(&lane(main)), !separate);
        rt.advance_to(0, 128).unwrap();
        let [eb, wb] = ids
            .each_ref()
            .map(|id| rt.trains().find(|t| &t.key.service_id == id).unwrap());
        assert!(matches!(eb.state, TrainState::Moving { entered_ms: 0, .. }));
        assert_eq!(wb.state, TrainState::Waiting { ready_ms: 0 });
        let replay = rt.preview_passenger_replay(1_000_000, 128).unwrap();
        assert!(replay.complete);
        let first = |i| {
            replay
                .completed_legs
                .iter()
                .find(|r| r.train.service_id == ids[i] && r.sequence == 0)
                .unwrap()
        };
        let last = |i| {
            replay
                .completed_legs
                .iter()
                .find(|r| r.train.service_id == ids[i] && r.sequence == 1)
                .unwrap()
        };
        assert_eq!(first(0).to, b.0.to_string());
        assert_eq!(first(1).to, if separate { s } else { b }.0.to_string());
        assert_eq!(last(0).to, c.0.to_string());
        assert_eq!(last(1).to, a.0.to_string());
        let west_entry = first(1).entered_ms;
        assert!(west_entry > first(0).arrived_ms);

        let advance = |world: &mut World, to| {
            let result = world
                .operation_mut()
                .unwrap()
                .runtime
                .advance_to(to, 128)
                .unwrap();
            assert!(result.reached_target);
            result.arrivals
        };
        let before = advance(&mut world, west_entry - 1);
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].at_ms, first(0).arrived_ms);
        let bytes = rosenfu_storage::encode(&world).unwrap();
        let mut reopened = rosenfu_storage::decode(&bytes).unwrap();
        assert_eq!(rosenfu_storage::encode(&reopened).unwrap(), bytes);
        assert!(advance(&mut reopened, west_entry).is_empty());
        let westbound = reopened
            .operation()
            .unwrap()
            .runtime
            .trains()
            .find(|train| train.key.service_id == ids[1])
            .unwrap();
        assert!(
            matches!(westbound.state, TrainState::Moving { entered_ms, .. } if entered_ms == west_entry)
        );
        let after = advance(&mut reopened, 1_000_000);
        assert_eq!(after.len(), 3);
        assert_eq!(after, advance(&mut world, 1_000_000));
        assert_eq!(world.operation_save(), reopened.operation_save());
        assert_eq!(reopened.operation().unwrap().runtime.trains().count(), 0);
        let live = before
            .into_iter()
            .chain(after)
            .map(|event| (event.train, event.station_id, event.at_ms))
            .collect::<std::collections::BTreeSet<_>>();
        let observed = replay
            .completed_legs
            .into_iter()
            .map(|leg| (leg.train, leg.to, leg.arrived_ms))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(live, observed);
    }
}
