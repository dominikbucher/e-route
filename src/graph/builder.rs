use std::fs::File;
use std::io::Read;
use byteorder::{LittleEndian, ReadBytesExt};
use spade::rtree::RTree;
use osmpbfreader::{OsmPbfReader, OsmObj};
use std::collections::{HashMap, HashSet};
use gluon;
use gluon::vm::api::{OpaqueValue, Hole, FunctionRef, IO};
use gluon::vm::thread::Thread;

use graph::core::{Graph, Node, Edge};
use graph::serializer::SerializableGraph;

use rust_geotiff::TIFF;

const MODULE_NAME: &'static &str = &"transport";
const FN_EDGE_VALID: &'static &str = &"transport.edge_valid";
const FN_EDGE_WEIGHT: &'static &str = &"transport.edge_weight";

pub struct GraphBuilder {}

impl GraphBuilder {
    pub fn build_from_pbf(pbf: &mut OsmPbfReader<File>, gluon_trans_scr: &mut File, dem: &TIFF) -> SerializableGraph {
        // Set up everything that is required for Gluon. The Gluon scripts are used
        // to specify which transport modes are extracted from the OSM file.
        type GluonEdge = (String);
        type GluonNode = (f64, f64);

        // Set up the Gluon VM, which compiles the scripts and makes their functions available.
        let gluon_vm = gluon::new_vm();
        let mut script = String::new();
        gluon_trans_scr.read_to_string(&mut script).unwrap();

        // Load the script and expose the required functions.
        gluon::Compiler::new()
            .load_script(&gluon_vm, MODULE_NAME, &script[..])
            .unwrap();
        let mut edge_valid: FunctionRef<fn (GluonEdge) -> bool> = gluon_vm
            .get_global(FN_EDGE_VALID)
            .unwrap();
        let mut edge_weight: FunctionRef<fn (GluonEdge, f64, GluonNode, GluonNode) -> f64> = gluon_vm
            .get_global(FN_EDGE_WEIGHT)
            .unwrap();

        // Set up graph building components.
        let mut important_nodes: HashSet<i64> = HashSet::new();
        let mut node_map: HashMap<i64, i64> = HashMap::new();
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();

        // First pass to get all important edges. In this first pass, they point from (OSM id
        // -> OSM id). Later, this is reduced to (Node id -> Node id).
        for obj in pbf.par_iter().map(Result::unwrap) {
            // In this version of e-route, we simply collect all ways that have the "highway"
            // tag. They are then passed to the Gluon function to determine the edge weight.
            if obj.is_way() && obj.tags().contains_key("highway") {
                let highway_tag = obj.tags().get("highway").unwrap();
                if edge_valid.call((highway_tag.to_string())).unwrap() {
                    // Get all the references to other ways.
                    for node in obj.way().unwrap().clone().nodes.windows(2) {
                        edges.push(Edge {
                            source: node[0].0,
                            target: node[1].0,
                            weight: 1.0,
                            highway_tag: highway_tag.to_string()
                        });
                        important_nodes.insert(node[0].0);
                        important_nodes.insert(node[1].0);
                    }
                }
            }
        }

        // Second pass to get all nodes.
        pbf.rewind().unwrap();
        for obj in pbf.par_iter().map(Result::unwrap) {
            if obj.is_node() && important_nodes.contains(&obj.id().node().unwrap().0) {
                let node_id = obj.id().node().unwrap().0;
                node_map.insert(node_id, nodes.len() as i64);
                nodes.push(Node {
                    id: node_id,
                    lon: obj.node().unwrap().lon(),
                    lat: obj.node().unwrap().lat()
                });
            }
        }

        // Finally, re-align node ids in edges.
        for edge in &mut edges {
            let new_source = node_map.get(&(edge.source as i64)).unwrap();
            let new_target = node_map.get(&(edge.target as i64)).unwrap();
            edge.source = *new_source;
            edge.target = *new_target;
            let source_node = &nodes[edge.source as usize];
            let target_node = &nodes[edge.target as usize];
            edge.weight = edge_weight
                .call((&edge.highway_tag).to_string(), source_node.dist_to(target_node),
                      (source_node.lon, source_node.lat),
                      (target_node.lon, target_node.lat))
                .unwrap() as f32;
        }

        SerializableGraph { edges: edges, nodes: nodes }
    }
}
