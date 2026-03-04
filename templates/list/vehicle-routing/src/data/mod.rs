//! Demo data — a small 10-customer CVRP instance.
//!
//! Replace with your own instance loading (file, API, database, …).

use crate::domain::ProblemData;

/// Returns a small demo instance: 1 depot + 10 customers, capacity 50.
pub fn demo_instance() -> (Box<ProblemData>, usize) {
    // Node 0 is the depot; nodes 1-10 are customers.
    let demands = vec![0, 10, 15, 20, 10, 15, 10, 20, 15, 10, 20];
    let n = demands.len();

    // Coordinates for distance computation: (x, y)
    let coords: Vec<(i64, i64)> = vec![
        (40, 40), // depot
        (20, 20), (30, 60), (60, 20), (80, 60), (50, 80),
        (70, 40), (10, 50), (90, 30), (50, 10), (30, 90),
    ];

    let distance_matrix = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    let dx = coords[i].0 - coords[j].0;
                    let dy = coords[i].1 - coords[j].1;
                    ((dx * dx + dy * dy) as f64).sqrt().round() as i64
                })
                .collect()
        })
        .collect();

    let n_vehicles = 3;

    let data = Box::new(ProblemData {
        capacity: 50,
        depot: 0,
        demands,
        distance_matrix,
    });

    (data, n_vehicles)
}
