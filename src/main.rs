extern crate byteorder;
extern crate time;
extern crate pbr;
extern crate iron;
extern crate params;
extern crate router;
extern crate mount;
extern crate persistent;
extern crate cgmath;
extern crate spade;
extern crate num;
extern crate staticfile;
extern crate postgres;
extern crate geojson;
extern crate rustc_serialize;
extern crate osmpbfreader;
extern crate rust_geotiff;


use std::env;
use iron::prelude::*;
use router::Router;
use mount::Mount;
use persistent::Read;
use staticfile::Static;
use std::path::Path;
use pbr::ProgressBar;
use rust_geotiff::TIFFReader;

mod graph;
mod spatialpoint;
mod endpoints;

use graph::Graph;
use endpoints::GraphPool;

fn wanted(obj: &osmpbfreader::OsmObj) -> bool {
    obj.is_way()
}

/// Main function and entry point to the program.
fn main() {
    // Startup.
    println!("\n<== Welcome to e-route ==>");
    let args: Vec<_> = env::args().collect();
    let start = time::now();

    // Loading the DEM data.
    println!("Reading DEM file from '{}'.", args[2]);
    let dem_path = std::path::Path::new(&args[2]);
    let img = TIFFReader.load(&args[2]).unwrap();
    println!("Finished reading DEM file.");

    // Loading the OSM pbf data.
    println!("Reading pbf file from '{}'.", args[1]);
    let pbf_path = std::path::Path::new(&args[1]);
    let pbf_file = std::fs::File::open(&pbf_path).unwrap();
    let mut pbf = osmpbfreader::OsmPbfReader::new(pbf_file);
    let pbf_data = pbf.get_objs_and_deps(wanted).unwrap();
    println!("Found {:?} ways in dataset.", pbf_data.len());

    // Processing data.
    println!("Starting graph construction.");
    let count = 1000;
    let mut pb = ProgressBar::new(count);
    pb.format("╢▌▌░╟");
    pb.inc();

    // let graph = Graph::new(&args[1]);
    let graph = Graph::new_from_db(&args[2], &args[3], &args[4], &args[5], &args[6],
                                   &args[7], &args[8]);
    println!("   duration: {}s\n", (time::now() - start).num_seconds());

    // Setting up the router for the web server.
    let mut router = Router::new();
    router.get("/route", endpoints::route_lat_lon, "route");
    router.get("/route-using-ids", endpoints::route_ids, "routeIds");
    router.get("/reachability", endpoints::reachability, "reachability");

    let mut mount = Mount::new();
    mount.mount("/api", router);
    mount.mount("/", Static::new(Path::new("./src/static/")));

    let mut chain = Chain::new(mount);
    chain.link_before(Read::<GraphPool>::one(graph));

    let address = ["127.0.0.1", &args[1]].join(":");
    Iron::new(chain).http(&*address).unwrap();
}
