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
extern crate clap;
extern crate config;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate bincode;
extern crate flate2;
extern crate gluon;

use iron::prelude::*;
use router::Router;
use mount::Mount;
use persistent::Read;
use staticfile::Static;
use std::path::Path;
use pbr::ProgressBar;
use rust_geotiff::TIFFReader;
use clap::{Arg, App};
use std::collections::HashMap;
use config::*;

mod graph;
mod spatialpoint;
mod endpoints;

use graph::builder::GraphBuilder;
use graph::serializer::SerializableGraph;
use graph::core::Graph;
use endpoints::GraphPool;

/// Main function and entry point to the program.
fn main() {
    let route_app = App::new("e-Router").version("0.1")
        .author("Dominik Bucher <dominik.bucher@gmail.com>")
        .about("A route calculation package, that focuses on electric mobility.")
        .arg(Arg::with_name("mode").required(true).index(1))
        .arg(Arg::with_name("config").value_name("FILE").takes_value(true))
        .get_matches();

    let filename = route_app.value_of("config").unwrap_or("default-conf.json");
    let mut settings = Config::default();
    settings.merge(File::from(Path::new(&filename))).unwrap();
    let settings_map = settings
        .try_into::<HashMap<String, String>>().unwrap();

    match route_app.value_of("mode") {
        Some("build-graph") => build_graph(settings_map),
        Some("run-server") => run_server(settings_map),
        _ => panic!("Unknown mode! Use one of 'build-graph', 'run-server'.")
    }
}

/// Graph building facility.
fn build_graph(settings_map: HashMap<String, String>) -> () {
    // Loading the DEM data.
    let dem_file = settings_map.get("dem_file").unwrap();
    println!("Reading DEM file from '{}'.", dem_file);
    //let img = TIFFReader.load(dem_file).unwrap();
    println!("Finished reading DEM file.");

    // Loading the OSM pbf data.
    let pbf_file = settings_map.get("osm_pbf_file").unwrap();
    println!("Reading pbf file from '{}'.", pbf_file);
    let pbf_path = std::path::Path::new(pbf_file);
    let pbf_file = std::fs::File::open(&pbf_path).unwrap();
    let mut pbf = osmpbfreader::OsmPbfReader::new(pbf_file);

    // Processing data.
    println!("Starting graph construction.");
    let count = 1000;
    let mut pb = ProgressBar::new(count);
    pb.format("╢▌▌░╟");
    pb.inc();

    let script_file = std::fs::File::open(
        &std::path::Path::new(settings_map.get("transport_mode").unwrap()));

    let graph = GraphBuilder::build_from_pbf(&mut pbf, &mut script_file.unwrap());
    let graph_file = settings_map.get("graph_file").unwrap();
    graph.write_to_file(graph_file);
    println!("Finished building graph.");
}

/// Exposes a graph to a public HTTP endpoint.
fn run_server(settings_map: HashMap<String, String>) -> () {
    println!("Running server");
    // let graph = Graph::new(&args[1]);
    /*let graph = Graph::load_from_db(settings_map.get("db_user").unwrap(),
                                   settings_map.get("db_password").unwrap(),
                                   settings_map.get("db_database").unwrap(),
                                   settings_map.get("db_database").unwrap(),
                                   settings_map.get("db_database").unwrap(),
                                   settings_map.get("db_database").unwrap(),
                                   settings_map.get("db_database").unwrap());*/

    let graph_file = settings_map.get("graph_file").unwrap();
    println!("Reading from {:?}.", graph_file);
    let serializable_graph = SerializableGraph::read_from_file(graph_file);
    println!("Finished reading. Building rtree now.");
    let graph = serializable_graph.to_graph();
    println!("Finished importing graph.");

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

    let address = [settings_map.get("server_host").unwrap().as_str(),
        settings_map.get("server_port").unwrap()].join(":");
    Iron::new(chain).http(&*address).unwrap();
    print!("Running server on {:?}", address);
}