use std::fs::File;
use byteorder::{LittleEndian, ReadBytesExt};
use spade::rtree::RTree;
use osmpbfreader::{OsmPbfReader, OsmObj};
use std::collections::{HashMap, HashSet};

use graph::core::Graph;
use graph::serializer::SerializableGraph;

// "highway" tags that are used for analysis.
const POSITIVE_TAGS: &'static [&str] = &["motorway", "trunk", "primary", "secondary", "tertiary",
    "unclassified", "residential", "service", "motorway_link", "trunk_link", "primary_link",
    "secondary_link", "tertiary_link"/*, "living_street", "pedestrian", "track", "road", "footway",
    "bridleway", "path", "cycleway"*/];

// "junction" tags that are excluded.
const NEGATIVE_TAGS: &'static [&str] = &["roundabout"];

pub struct GraphBuilder {}

impl GraphBuilder {
    fn wanted(obj: &OsmObj) -> bool {
        obj.is_way()
    }

    pub fn build_from_pbf(pbf: &mut OsmPbfReader<File>) -> () {
        let mut obj_count = 0;
        let mut adjacent_nodes: HashMap<i64, HashSet<i64>> = HashMap::new();

        for obj in pbf.par_iter().map(Result::unwrap) {
            // Not so sure how great this is, in combination with par_iter(). Seems to work for now ...
            obj_count += 1;
            if obj.is_way() && obj.tags().contains_key("highway") &&
                POSITIVE_TAGS.contains(&&obj.tags().get("highway").unwrap()[..]) {
                if !(obj.tags().contains_key("junction") &&
                    NEGATIVE_TAGS.contains(&&obj.tags().get("junction").unwrap()[..])) {
                    // Get all the references to other ways.
                    for node in obj.way().unwrap().clone().nodes.windows(2) {
                        adjacent_nodes.entry(node[0].0).or_insert(HashSet::new()).insert(node[1].0);
                        adjacent_nodes.entry(node[1].0).or_insert(HashSet::new()).insert(node[0].0);
                    }
                }
            }
        }

        println!("Object count: {:?}", obj_count);

        let graph = SerializableGraph { edges: Vec::new(), nodes: Vec::new() };
        graph.write_to_file("data/graph.bin.gz");
    }
}
