use rosenfu_core::world::World;
use std::{env, error::Error, ffi::OsString, io};

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: rosenfu-cli [world new <path> | world inspect <path>]",
    )
}
fn run(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("{}", serde_json::to_string(&rosenfu_core::engine_info())?);
        return Ok(());
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
                    "stations": data.stations.len(), "tracks": data.tracks.len(), "routes": data.routes.len(),
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
