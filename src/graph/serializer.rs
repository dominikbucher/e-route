use std::fs::File;
use std::io::{BufWriter, BufReader};

use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use flate2::Compression;
use bincode::{serialize_into, deserialize_from, Infinite};

use graph::core::{Graph, Edge, Node};

/// Contains parts of a graph that can be serialized.
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableGraph {
    /// All the edges contained in the graph.
    pub edges: Vec<Edge>,
    /// All the nodes contained in this graph.
    pub nodes: Vec<Node>
}

impl SerializableGraph {
    pub fn write_to_file(&self, filename: &str) -> () {
        let writer = BufWriter::new(File::create(filename).unwrap());
        let mut encoder = ZlibEncoder::new(writer, Compression::Best);
        let encoded = serialize_into(&mut encoder, &self, Infinite).unwrap();
    }

    pub fn read_from_file(&self, filename: &str) -> () {
        let reader = BufReader::new(File::open(filename).unwrap());
        let mut decoder = ZlibDecoder::new(reader);
        let decoded: SerializableGraph = deserialize_from(&mut decoder, Infinite).unwrap();
    }
}