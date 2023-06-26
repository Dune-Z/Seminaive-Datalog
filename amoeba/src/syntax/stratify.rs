use std::collections::{HashMap, HashSet};
use petgraph::{algo, graphmap::DiGraphMap};

#[derive(Clone)]
pub struct Stratum {
    pub strata: Vec<HashSet<String>>,
    pub levels: HashMap<String, usize>
}

impl Stratum {
    pub fn new(relations: HashSet<String>, dependencies: HashSet<(&String, &String)>) -> Self {
        let mut graph = DiGraphMap::new();
        for node in relations.iter() {
            graph.add_node(node);
        }
        for edge in dependencies.iter() {
            graph.add_edge(edge.0, edge.1, ());
        }
        let scc = algo::kosaraju_scc(&graph);
        let mut strata = Vec::new();
        let mut levels = HashMap::new();
        for (i, component) in scc.into_iter().enumerate() {
            let mut stratum = HashSet::new();
            for node in component {
                stratum.insert(node.to_string());
                levels.insert(node.to_string(), i);
            }
            strata.push(stratum);
        }
        Self { strata, levels }
    }

    pub fn get_level(&self, relation: &String) -> usize {
        *self.levels.get(relation).expect("relation not found")
    }
}