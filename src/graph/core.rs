use std;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use byteorder::{LittleEndian, ReadBytesExt};
use pbr::ProgressBar;
use spade::rtree::RTree;
use cgmath::Point2;
use postgres::{Connection, TlsMode};

use spatialpoint::SpatialPoint;

// Inspired by http://codegists.com/snippet/rust/bellmanrs_tristramg_rust.

/// Holds a single node, containing the OSM id, longitude, and latitude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// The OSM id associated with this node.
    pub id: i64,
    /// The longitude of this node.
    pub lon: f64,
    /// The latitude of this node.
    pub lat: f64,
}

/// Holds a single edge, containing the source node, the target node,
/// and the edge weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Where this edge starts.
    pub source: i64,
    /// Where this edge ends.
    pub target: i64,
    /// The weight of this edge.
    pub weight: f32,
    /// The tag of this edge.
    pub highway_tag: String,
}

/// Contains a whole graph.
pub struct Graph {
    /// All the edges contained in the graph.
    pub edges: Vec<Edge>,
    /// All the nodes contained in this graph.
    pub nodes: Vec<Node>,
    /// An R tree for quick access to the nodes, given a longitude and latitude.
    pub rtree: RTree<SpatialPoint>,
}

/// Implementation of node.
impl Node {
    /// Reads a node from an OSRM file.
    fn from_osrm(reader: &mut BufReader<&File>) -> Node {
        let lon = reader.read_i32::<LittleEndian>().unwrap();
        let lat = reader.read_i32::<LittleEndian>().unwrap();
        let id = reader.read_u64::<LittleEndian>().unwrap() as i64;
        let _ = reader.seek(SeekFrom::Current(8));

        Node {
            id: id,
            lon: lon as f64 / 1e6,
            lat: lat as f64 / 1e6,
        }
    }

    /// Computes the Haversine distance to another node.
    pub fn dist_to(&self, other: &Node) -> f64 {
        let earth_radius = 6371.0;
        let lat1_rad = self.lat.to_radians();
        let lon1_rad = self.lon.to_radians();
        let lat2_rad = other.lat.to_radians();
        let lon2_rad = other.lon.to_radians();

        let tmp = ((lat2_rad - lat1_rad) / 2.00).sin().powf(2.0) +
            lat1_rad.cos() * lat2_rad.cos() * ((lon2_rad - lon1_rad) / 2.0).sin().powf(2.0);
        let tmp = 2.0 * ((tmp).sqrt().atan2((1.0 - tmp).sqrt()));
        earth_radius * tmp
    }
}

/// Implementation of edge.
impl Edge {
    /// Reads an edge from an OSRM file.
    fn from_osrm(reader: &mut BufReader<&File>) -> Edge {
        let source = reader.read_u32::<LittleEndian>().unwrap() as i64;
        let target = reader.read_u32::<LittleEndian>().unwrap() as i64;
        let _ = reader.seek(SeekFrom::Current(4));
        let weight = reader.read_u32::<LittleEndian>().unwrap();
        let _ = reader.seek(SeekFrom::Current(8));

        Edge {
            source: source,
            target: target,
            weight: weight as f32,
            highway_tag: "".to_string(),
        }
    }
}

/// Implementation of graph.
impl Graph {
    /// Creates a new graph, by reading an OSRM file. This also adds and returns an OSM id
    /// as the starting point for a later bellman-ford query.
    pub fn load_from_osrm(file: &String) -> Graph {
        let file = File::open(file).unwrap();

        let mut reader = BufReader::new(&file);
        let _ = reader.seek(SeekFrom::Start(152));

        // First, we read in all nodes.
        let nodes_count = reader.read_u32::<LittleEndian>().unwrap() as usize;
        println!(" ˪— Reading {:?} nodes", nodes_count);
        let mut nodes = Vec::with_capacity(nodes_count);
        let mut n_pb = ProgressBar::new(nodes_count as u64);
        for i in 0..nodes_count {
            let node = Node::from_osrm(&mut reader);
            nodes.push(node);
            if i % 1000 == 0 {
                n_pb.add(1000);
            }
        }

        // Then, we continue with all edges.
        let edges_count = reader.read_u32::<LittleEndian>().unwrap() as usize;
        println!(" ˪– Reading {:?} edges", edges_count);
        let mut edges = Vec::with_capacity(edges_count);
        let mut e_pb = ProgressBar::new(edges_count as u64);
        for i in 0..edges_count {
            edges.push(Edge::from_osrm(&mut reader));
            if i % 1000 == 0 {
                e_pb.add(1000);
            }
        }

        // Finally, we build an R tree for quick access.
        let mut rtree = RTree::new();
        for n in nodes.iter() {
            let p = SpatialPoint::new(Point2::new(n.lon, n.lat), n.id);
            rtree.insert(p);
        }

        Graph { edges: edges, nodes: nodes, rtree: rtree }
    }

    /// Loads a graph from a Postgres database.
    pub fn load_from_db(uname: &String, pw: &String, db: &String,
                        ways_vert_table: &String, ways_table: &String,
                        weight: &String, weight_rev: &String) -> Graph {
        let conn_str = format!("postgres://{}:{}@localhost/{}", uname, pw, db);
        let conn = Connection::connect(conn_str, TlsMode::None).unwrap();

        let mut nodes = Vec::with_capacity(1000);
        let select_str_vert = format!("SELECT id, lon, lat FROM {} ORDER BY id", ways_vert_table);
        for row in &conn.query(&select_str_vert, &[]).unwrap() {
            let osm_id: i64 = row.get(0);
            let lon_raw: f64 = row.get(1);
            let lat_raw: f64 = row.get(2);

            let node = Node {
                id: osm_id,
                lon: lon_raw,
                lat: lat_raw,
            };
            nodes.push(node);
        }

        let mut edges = Vec::with_capacity(1000);
        let select_str = format!("SELECT source, target, {}, {} FROM {}", weight, weight_rev, ways_table);
        for row in &conn.query(&select_str, &[]).unwrap() {
            let source_id: i64 = row.get(0);
            let target_id: i64 = row.get(1);
            let weight_raw: f64 = row.get(2);
            let weight_raw_rev: f64 = row.get(3);

            // When inserting, we simply subtract 1, so that the IDs map to those of the nodes.
            // This comes from the fact that the Rust vector is 0-indexed, but in Postgres,
            // the IDs start with 1.
            let edge = Edge {
                source: source_id - 1,
                target: target_id - 1,
                weight: weight_raw as f32,
                highway_tag: "".to_string(),
            };
            edges.push(edge);

            // We also insert edges for every backward edge.
            let edge = Edge {
                source: target_id - 1,
                target: source_id - 1,
                weight: weight_raw_rev as f32,
                highway_tag: "".to_string(),
            };
            edges.push(edge);
        }

        // Finally, we build an R tree for quick access.
        let mut rtree = RTree::new();
        for n in nodes.iter() {
            let p = SpatialPoint::new(Point2::new(n.lon, n.lat), n.id);
            rtree.insert(p);
        }

        Graph { edges: edges, nodes: nodes, rtree: rtree }
    }

    /// Gets the node IDs from a longitude and latitude.
    pub fn get_id_from_lon_lat(&self, lon: f64, lat: f64) -> i64 {
        let nearest = self.rtree.nearest_neighbor(&Point2::new(lon, lat)).unwrap();
        nearest.id
    }

    /// Gets the internal ID from an OSM id.
    fn get_id_from_osm(&self, osm_id: i64) -> usize {
        self.nodes.iter().position(|r| r.id == osm_id).unwrap()
    }

    /// Gets the location from an internal id. Returns a vector containing
    /// longitude and latitude.
    fn get_loc_from_id(&self, id: usize) -> Vec<f64> {
        vec![self.nodes[id].lon, self.nodes[id].lat]
    }

    /// Performs a routing request from source to target.
    pub fn route(&self, source: i64, target: i64) -> (Vec<Vec<f64>>, f32) {
        let source_id = self.get_id_from_osm(source);
        let target_id = self.get_id_from_osm(target);
        let (pred, dist) = self.bellman(source_id);
        let max_length = self.edges.len();

        println!(" ˪— Backtracking from {}, having {} edges. Total cost: {}.",
                 target_id, max_length, dist[target_id]);
        let mut trace = Vec::new();
        let mut current_node = target_id;
        trace.push(self.get_loc_from_id(current_node));
        let mut count = 0;
        while current_node != source_id {
            current_node = pred[current_node];
            trace.push(self.get_loc_from_id(current_node));

            count = count + 1;
            // Make sure this doesn't run forever.
            if count > max_length {
                current_node = source_id;
            }
        }

        (trace, dist[target_id])
    }

    /// Computes the reachability of all nodes in the graph, and returns those which
    /// are reachable. Returns a vector of vectors, where the coordinates are as follows:
    /// longitude, latitude, remaining_energy.
    pub fn reachability(&self, source: i64, capacity: f32) -> Vec<Vec<f64>> {
        let source_id = self.get_id_from_osm(source);
        let (pred, dist) = self.bellman(source_id);
        let max_length = self.nodes.len();

        println!(" ˪— Assessing all {} nodes to select feasible ones.", max_length);
        let mut trace = Vec::new();
        for (i, node) in dist.iter().enumerate() {
            if capacity - node >= 0.0 {
                let mut loc = self.get_loc_from_id(i);
                loc.push((capacity - node) as f64);
                trace.push(loc);
            }
        }

        trace
    }

    /// Runs the Bellman Ford algorithm on the graph. Returns a tuple, containing a vector of
    /// predecessors and a vector of distances to the source node.
    fn bellman(&self, source: usize) -> (Vec<usize>, Vec<f32>) {
        let nodes_count = self.nodes.len();
        let max_length = self.edges.len();
        println!(" ˪— Starting from {}, having {} nodes.", source, nodes_count);
        let mut pred = (0..nodes_count).collect::<Vec<_>>();
        let mut dist = std::iter::repeat(std::f32::MAX).take(nodes_count).collect::<Vec<_>>();
        dist[source] = 0.0;
        let mut count = 0;

        let mut improvement = true;
        while improvement {
            improvement = false;
            for edge in &self.edges {
                let source_dist = dist[edge.source as usize];
                let target_dist = dist[edge.target as usize];

                if source_dist != std::f32::MAX && source_dist + edge.weight < target_dist {
                    dist[edge.target as usize] = source_dist + edge.weight;
                    pred[edge.target as usize] = edge.source as usize;
                    improvement = true;
                }

                // This would be needed for undirected edges, as we'd have to follow every
                // edge both ways in that case.
                // if target_dist != std::f32::MAX && target_dist + edge.weight < source_dist {
                //     dist[edge.source as usize] = target_dist + edge.weight;
                //     pred[edge.source as usize] = edge.target as usize;
                //     improvement = true;
                // }
            }
            count = count + 1;

            // Make sure this doesn't run forever.
            if count > max_length {
                improvement = false;
            }
        }
        println!(" ˪— Bellman iterations: {}", count);

        (pred, dist)
    }

    /// Runs the Djikstra algorithm on the graph. Returns a tuple, containing a vector of
    /// predecessors and a vector of distances to the source node.
    fn djikstra(&self, source: usize, target: usize) -> Option<f32> {
        let mut dists: HashMap<usize, f32> = HashMap::new();
        let mut heap = BinaryHeap::new();

        dists.insert(source, 0.0);
        heap.push(State { cost: 0.0, position: source });

        while let Some(State { cost, position }) = heap.pop() {
            if position == target { return Some(cost); }
            if cost > dists[&position] { continue; }

            for edge in &self.edges {
                if (edge.source == position as i64) {
                    let next = State { cost: cost + edge.weight, position: edge.target as usize };
                    if !dists.contains_key(&next.position) {
                        dists.insert(next.position, next.cost);
                        heap.push(next);
                    } else if next.cost < dists[&next.position] {
                        dists.insert(next.position, next.cost);
                        heap.push(next);
                    }
                }
            }
        }

        None
    }
}

#[derive(Copy, Clone, PartialEq)]
struct State {
    cost: f32,
    position: usize,
}

impl Eq for State {}

// The priority queue depends on `Ord`.
// Explicitly implement the trait so the queue becomes a min-heap
// instead of a max-heap.
impl Ord for State {
    fn cmp(&self, other: &State) -> Ordering {
        // Notice that the we flip the ordering on costs.
        // In case of a tie we compare positions - this step is necessary
        // to make implementations of `PartialEq` and `Ord` consistent.
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Less)
            .then_with(|| self.position.cmp(&other.position))
    }
}

// `PartialOrd` needs to be implemented as well.
impl PartialOrd for State {
    fn partial_cmp(&self, other: &State) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}