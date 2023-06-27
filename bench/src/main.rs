use std::time::Instant;
use crepe::crepe;

crepe! {
    @input
    struct Edge(i32, i32);
    @output
    struct Reachable(i32, i32);

    Reachable(x, y) <- Edge(x, y);
    Reachable(x, z) <- Edge(x, y), Reachable(y, z);
}

fn main() {
    let now = Instant::now();
    let mut runtime = Crepe::new();
    // read nodes number and edges number from command line
    let args = std::env::args().collect::<Vec<String>>();
    let n_nodes = args[1].parse::<i32>().unwrap();
    let n_edges = args[2].parse::<i32>().unwrap();
    // randomly generate 1000 edges in 500 nodes for Edge
    let mut edges = Vec::new();
    for _ in 0..n_edges {
        let x = rand::random::<i32>() % n_nodes;
        let y = rand::random::<i32>() % n_nodes;
        edges.push(Edge(x, y));
    }
    runtime.extend(edges);
    let (_reachable, ) = runtime.run();
    let elapsed = now.elapsed();
    // for Reachable(x, y) in reachable.iter() {
    //     println!("Reachable({}, {})", x, y);
    // }
    println!("{} reachable pairs", _reachable.len());
    println!("{}.{:03}s", elapsed.as_secs(), elapsed.subsec_millis());
}
