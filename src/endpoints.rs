extern crate time;
extern crate iron;
extern crate geojson;

use iron::prelude::*;
use iron::typemap::Key;
use persistent::Read;
use graph::core::Graph;
use std::collections::BTreeMap;
use rustc_serialize::json::ToJson;
use geojson::{Feature, FeatureCollection, GeoJson, Geometry};

/// A pool that abstracts over the graph, and makes it available to all requests.
pub struct GraphPool;

impl Key for GraphPool { type Value = Graph; }

/// Transforms the result of a route calculation into a GeoJSON, convenient for sending
/// over the Internet.
fn route_res_to_geojson(lat_lons: Vec<Vec<f64>>, cost: f32) -> String {
    let geometry = Geometry::new(
        geojson::Value::LineString(lat_lons.iter().map(|x|
            x.iter().map(|&y| y).collect::<Vec<_>>()
        ).collect::<Vec<_>>())
    );

    let mut properties = BTreeMap::new();
    properties.insert(
        String::from("total_cost"),
        cost.to_json(),
    );

    let geojson = GeoJson::Feature(Feature {
        crs: None,
        bbox: None,
        geometry: Some(geometry),
        id: None,
        properties: Some(properties),
    });

    geojson.to_string()
}

/// Transforms the result of a reachability calculation to a GeoJSON string, ready
/// to be processed in the frontend.
fn reachability_res_to_geojson(lat_lon_caps: Vec<Vec<f64>>) -> String {
    let mut features = Vec::new();
    for lat_lon in lat_lon_caps {
        let mut props = BTreeMap::new();
        props.insert(
            String::from("capacity_remaining"),
            lat_lon[2].to_json(),
        );

        features.push(Feature {
            crs: None,
            bbox: None,
            geometry: Some(Geometry::new(
                geojson::Value::Point(lat_lon[0..2].iter()
                    .map(|&y| y as f64).collect::<Vec<_>>())
            )),
            id: None,
            properties: Some(props)
        });
    }

    let geojson = GeoJson::FeatureCollection(FeatureCollection {
        crs: None,
        bbox: None,
        features: features,
    });

    geojson.to_string()
}

/// Computes a route, given a start and end latitude and longitude.
pub fn route_lat_lon(req: &mut Request) -> IronResult<Response> {
    let graph = req.get::<Read<GraphPool>>().unwrap();
    use params::{Params, Value};
    let map = req.get_ref::<Params>().unwrap();

    match (map.find(&["source-lon"]), map.find(&["source-lat"]),
           map.find(&["target-lon"]), map.find(&["target-lat"])) {
        (Some(&Value::String(ref source_lon)), Some(&Value::String(ref source_lat)),
            Some(&Value::String(ref target_lon)), Some(&Value::String(ref target_lat))) => {
            let bellman_start = time::now();
            println!("Starting Bellman-Ford ...");
            let source_id = graph.get_id_from_lon_lat(source_lon.parse::<f64>().unwrap(),
                                                      source_lat.parse::<f64>().unwrap());
            let target_id = graph.get_id_from_lon_lat(target_lon.parse::<f64>().unwrap(),
                                                      target_lat.parse::<f64>().unwrap());
            let res = graph.route(source_id, target_id);
            println!(" ˪— duration: {}s\n", (time::now() - bellman_start).num_seconds());

            Ok(Response::with((iron::status::Ok, route_res_to_geojson(res.0, res.1))))
        }
        _ => Ok(Response::with(iron::status::NotFound))
    }
}

/// Computes a route, given a start and end OSM ID.
pub fn route_ids(req: &mut Request) -> IronResult<Response> {
    let graph = req.get::<Read<GraphPool>>().unwrap();
    use params::{Params, Value};
    let map = req.get_ref::<Params>().unwrap();

    match (map.find(&["source-id"]), map.find(&["target-id"])) {
        (Some(&Value::String(ref source_id)), Some(&Value::String(ref target_id))) => {
            let bellman_start = time::now();
            println!("Starting Bellman-Ford ...");
            let res = graph.route(source_id.parse::<i64>().unwrap(),
                                  target_id.parse::<i64>().unwrap());
            println!(" ˪— duration: {}s\n", (time::now() - bellman_start).num_seconds());

            Ok(Response::with((iron::status::Ok, route_res_to_geojson(res.0, res.1))))
        }
        _ => Ok(Response::with(iron::status::NotFound))
    }
}

/// Returns all reachable nodes in a vicinity. This can be a lot, so take care!
pub fn reachability(req: &mut Request) -> IronResult<Response> {
    let graph = req.get::<Read<GraphPool>>().unwrap();
    use params::{Params, Value};
    let map = req.get_ref::<Params>().unwrap();

    match (map.find(&["source-lon"]), map.find(&["source-lat"]), map.find(&["capacity"])) {
        (Some(&Value::String(ref source_lon)), Some(&Value::String(ref source_lat)),
            Some(&Value::String(ref capacity))) => {
            let bellman_start = time::now();
            println!("Starting Reachability Bellman-Ford ...");
            let source_id = graph.get_id_from_lon_lat(source_lon.parse::<f64>().unwrap(),
                                                      source_lat.parse::<f64>().unwrap());
            let res = graph.reachability(source_id, capacity.parse::<f32>().unwrap());
            println!(" ˪— duration: {}s\n", (time::now() - bellman_start).num_seconds());

            Ok(Response::with((iron::status::Ok, reachability_res_to_geojson(res))))
        }
        _ => Ok(Response::with(iron::status::NotFound))
    }
}
