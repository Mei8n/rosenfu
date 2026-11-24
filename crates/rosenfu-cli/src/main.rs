use rosenfu_core::world::World;
use std::{env, error::Error, ffi::OsString, io, path::Path};

mod road_proof;

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: rosenfu-cli [world new <path> | world inspect <path> | world road-coverage <path> <min-x> <min-y> <max-x> <max-y> | world verify-road-proof <approved.json> <sha256> <capture.rosenfu>]",
    )
}
fn run(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("{}", serde_json::to_string(&rosenfu_core::engine_info())?);
        return Ok(());
    }
    if let [world, action, approved, digest, capture] = args.as_slice() {
        if world == "world" && action == "verify-road-proof" {
            return road_proof::verify(
                Path::new(approved),
                digest.to_str().ok_or_else(usage)?,
                Path::new(capture),
            );
        }
    }
    if let [world, action, path, min_x, min_y, max_x, max_y] = args.as_slice() {
        if world == "world" && action == "road-coverage" {
            let tile = |value: &OsString| {
                value
                    .to_str()
                    .and_then(|text| text.parse::<u32>().ok())
                    .ok_or_else(usage)
            };
            let bounds = rosenfu_storage::TileBounds::new(
                tile(min_x)?,
                tile(min_y)?,
                tile(max_x)?,
                tile(max_y)?,
            )
            .ok_or_else(usage)?;
            let result = rosenfu_storage::read_road_coverage(Path::new(path), bounds)?;
            println!("{}", serde_json::to_string(&result)?);
            return Ok(());
        }
    }
    if args.len() != 3 || args[0] != "world" {
        return Err(usage().into());
    }
    match args[1].to_str() {
        Some("new") => {
            rosenfu_storage::create_world(std::path::Path::new(&args[2]), &World::default())?;
            println!("{}", serde_json::json!({ "created": true }));
        }
        Some("inspect") => {
            let world = rosenfu_storage::load_world(std::path::Path::new(&args[2]))?;
            let data = world.snapshot();
            println!(
                "{}",
                serde_json::json!({
                    "players": data.players.len(), "companies": data.companies.len(),
                    "stations": data.stations.len(), "alignments": data.alignments.len(), "routes": data.routes.len(),
                    "services": data.services.len(), "next_id": data.next_id.to_string()
                })
            );
        }
        _ => return Err(usage().into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run(env::args_os().skip(1).collect()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
