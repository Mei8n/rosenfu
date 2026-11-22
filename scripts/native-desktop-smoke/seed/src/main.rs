#[allow(dead_code, unused_imports)]
#[path = "../../exact/src-tauri/src/session.rs"]
mod session;

use rosenfu_core::{
    operation::{OperationError, Timetable, Vehicle},
    road_transport::{native_road_regions, PilotRoadMode, TramPlacement},
    world::{
        AlignmentConnection, AlignmentEndpoint::{End,Start}, AlignmentEndpointRef, GeoPoint,
        OperationServiceConfig, RouteSegment, TrackType, ViewState, World,
    },
};
use rosenfu_storage::{create_save_slot, list_save_slots, load_world, save_slot_path, save_world};
use session::{RoadTramEdit, WorldEditor};

const DATASET:&str="dejima-osm-se-tram-lab";
const START:&str="lab-dejima:se-clip-a";
const END:&str="lab-dejima:se-clip-b";
const EDGE:&str="lab-dejima:osm-128993698-v12-17-18-se";
const SLOT:&str="Dejima seeded through service";
const SERVICE:&str="Native Dejima through tram";

fn main(){
 let temp=std::path::PathBuf::from(std::env::args().nth(1).expect("temporary profile root"));
 assert!(temp.is_dir() && temp.file_name().and_then(|s|s.to_str()).unwrap_or("").starts_with("rosenfu-dejima-"));
 let root=temp.join("data/io.github.mei8n.rosenfu/prototype");
 assert!(!root.exists(),"fixture only writes a fresh isolated save root");
 let regions=serde_json::to_value(native_road_regions().unwrap()).unwrap();
 assert!(regions.as_array().unwrap().iter().any(|r|r["id"]==DATASET && r["lab_only"]==true));
 let mut editor=WorldEditor::new();
 editor.create_new_world("Dejima QA company".into()).unwrap();
 let installed=editor.install_native_road_pilot(DATASET).unwrap();
 let road=&installed.road_transport.edges[0];
 assert_eq!(road.id,EDGE);
 let points=road.geometry.clone();
 assert_eq!(points.len(),2);
 let [a,b]=[points[0],points[1]];
 let build=|from:&str,to:&str|RoadTramEdit::Build{
     from:from.into(),to:to.into(),name:"Dejima exact clip".into(),shared:true};
 assert!(editor.edit_road_tram(build(END,START)).is_err());
 assert_eq!(editor.snapshot(),installed);
 assert!(installed.road_transport.native_shortest_path(&installed,START,END,PilotRoadMode::Bus).is_err());
 let built=editor.edit_road_tram(build(START,END)).unwrap();
 let clip=built.alignments[0].id;
 let edit=|shared|RoadTramEdit::ChangePlacement{alignment_id:clip,from:START.into(),to:END.into(),shared};
 let detached=editor.edit_road_tram(edit(false)).unwrap();
 assert_eq!(detached.alignments,built.alignments);
 assert_eq!(detached.road_transport.tram_sections[0].placement,TramPlacement::Dedicated);
 let attached=editor.edit_road_tram(edit(true)).unwrap();
 assert_eq!(attached,built);
 assert_eq!(attached.road_transport.tram_sections[0].placement,TramPlacement::SharedRoad{edges:vec![EDGE.into()]});
 let owner=attached.companies[0].id;
 let mut world=World::new(attached).unwrap();
 let extend=|base:GeoPoint, other:GeoPoint|GeoPoint {
   latitude:base.latitude+5.0*(base.latitude-other.latitude),
   longitude:base.longitude+5.0*(base.longitude-other.longitude),
 };
 let inbound=world.build_alignment_typed(owner,"Synthetic inbound approach".into(),vec![extend(a,b),a],TrackType::Tram).unwrap();
 let outbound=world.build_alignment_typed(owner,"Synthetic outbound approach".into(),vec![b,extend(b,a)],TrackType::Tram).unwrap();
 let origin=world.place_station_on_alignment(owner,"Synthetic Tram Origin".into(),inbound,0.5,40.,8.).unwrap();
 let destination=world.place_station_on_alignment(owner,"Synthetic Tram Destination".into(),outbound,0.5,40.,8.).unwrap();
 let link=|a,b|AlignmentConnection{a:AlignmentEndpointRef{alignment_id:a,endpoint:End},
                                   b:AlignmentEndpointRef{alignment_id:b,endpoint:Start}};
 let links=[link(inbound,clip),link(clip,outbound)];
 for link in &links {world.connect_alignment_endpoints(owner,link.a,link.b).unwrap();}
 let create=|w:&mut World,name:&str,from,to,connections|{
   let seg=vec![RouteSegment::ConnectedAlignments{from,to,connections}];
   let stops=w.preview_service_path(from,seg.clone()).unwrap().occurrences.into_iter().map(|o|o.reference).collect();
   w.add_service_path(owner,name.into(),from,seg,stops).unwrap()
 };
 let service=create(&mut world,SERVICE,origin,destination,links.to_vec());
 let cfg=|service_id|OperationServiceConfig{service_id,timetable:Timetable{first_departure_ms:0,last_departure_ms:0,headway_ms:60_000},
   vehicle:Vehicle{max_speed_kmh:35.,acceleration_mps2:1.,braking_mps2:1.2,length_m:12.},dwell_ms:10_000};
 let config=cfg(service);
 let spec=world.compile_operation(&config).unwrap();
 assert_eq!(spec.legs.len(),1,"the genuine clipped direction must not get a reverse loop");
 let resources=format!("rail:{}:section:0:lane:",clip.0);
 let clip_section=spec.legs[0].sections.iter().filter(|s|s.resources.iter().any(|r|r.starts_with(&resources))).collect::<Vec<_>>();
 assert_eq!(clip_section.len(),1);
 assert!((30.0..32.0).contains(&clip_section[0].length_m));
 let mut snap=world.snapshot().clone();
 snap.view_state=Some(ViewState{latitude:(a.latitude+b.latitude)/2.,longitude:(a.longitude+b.longitude)/2.,zoom:17.});
 world.replace(snap).unwrap();
 let (slot,_)=create_save_slot(&root,SLOT,&world).unwrap();
 let slot_path=save_slot_path(&root,&slot.id).unwrap();
 assert_eq!(list_save_slots(&root).unwrap().len(),1);
 let reopened_seed=load_world(&slot_path).unwrap();
 assert_eq!(reopened_seed.snapshot(),world.snapshot());
 assert!(reopened_seed.operation_save().is_none());
 let wire=serde_json::to_value(reopened_seed.snapshot()).unwrap();
 assert_eq!(wire["service_paths"][0]["service_id"],service.0.to_string());
 assert_eq!(wire["service_paths"][0]["legs"][0]["segment"]["kind"],"connected_alignments");
 assert_eq!(wire["road_transport"]["tram_sections"][0]["placement"]["kind"],"shared_road");

 let mut operated=reopened_seed;
 let reverse=create(&mut operated,"Reverse native negative control",destination,origin,
   links.iter().rev().map(|l|AlignmentConnection{a:l.b,b:l.a}).collect());
 assert_eq!(operated.compile_operation(&cfg(reverse)),Err(OperationError::InvalidPath));
 operated.configure_operation(vec![config]).unwrap();
 let runtime=&mut operated.operation_mut().unwrap().runtime;
 let early=runtime.advance_to(5_000,100).unwrap();
 assert!(early.arrivals.is_empty());runtime.set_paused(true);
 let before=operated.operation_save().unwrap();
 assert_eq!(before.runtime.trains.len(),1);
 let proof_path=temp.join("seed-active-proof.rosenfu");
 save_world(&proof_path,&operated).unwrap();
 let mut resumed=load_world(&proof_path).unwrap();
 assert_eq!(resumed.operation_save().unwrap(),before);
 let a=operated.operation_mut().unwrap().runtime.advance_to(60_000,100).unwrap();
 let b=resumed.operation_mut().unwrap().runtime.advance_to(60_000,100).unwrap();
 assert_eq!(a,b);
 assert_eq!(b.arrivals.len(),1);
 assert_eq!(b.arrivals[0].station_id,destination.0.to_string());
 println!("{}",serde_json::json!({"slot":SLOT,"service":SERVICE,"clipId":clip.0.to_string(),"serviceId":service.0.to_string(),
  "clipMetres":clip_section[0].length_m,"arrivalMs":b.arrivals[0].at_ms}));
}
