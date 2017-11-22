use std::fs::File;
use byteorder::{LittleEndian, ReadBytesExt};
use spade::rtree::RTree;
use osmpbfreader::{OsmPbfReader, OsmObj};
use std::collections::{HashMap, HashSet};

use graph::core::{Graph, Node, Edge};
use graph::serializer::SerializableGraph;

// "highway" tags that are used for analysis.
const POSITIVE_TAGS: &'static [&str] = &["motorway", "trunk", "primary", "secondary", "tertiary",
    "unclassified", "residential", "service", "motorway_link", "trunk_link", "primary_link",
    "secondary_link", "tertiary_link"/*, "living_street", "pedestrian", "track", "road", "footway",
    "bridleway", "path", "cycleway"*/];

// "junction" tags that are excluded.
const NEGATIVE_TAGS: &'static [&str] = &[/*"roundabout"*/];

pub struct GraphBuilder {}

impl GraphBuilder {
    fn wanted(obj: &OsmObj) -> bool {
        obj.is_way()
    }

    pub fn build_from_pbf(pbf: &mut OsmPbfReader<File>) -> SerializableGraph {
        let mut important_nodes: HashSet<i64> = HashSet::new();
        let mut node_map: HashMap<i64, usize> = HashMap::new();
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();

        // First pass to get all important edges. In this first pass, they point from (OSM id
        // -> OSM id). Later, this is reduced to (Node id -> Node id).
        for obj in pbf.par_iter().map(Result::unwrap) {
            if obj.is_way() && obj.tags().contains_key("highway") &&
                POSITIVE_TAGS.contains(&&obj.tags().get("highway").unwrap()[..]) {
                if !(obj.tags().contains_key("junction") &&
                    NEGATIVE_TAGS.contains(&&obj.tags().get("junction").unwrap()[..])) {
                    // Get all the references to other ways.
                    for node in obj.way().unwrap().clone().nodes.windows(2) {
                        edges.push(Edge {
                            source: node[0].0 as usize,
                            target: node[1].0 as usize,
                            weight: 1.0
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
                node_map.insert(node_id, nodes.len());
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
        }

        // Gluon code here.

        SerializableGraph { edges: edges, nodes: nodes }
    }
}
