//! Clarke-Wright savings construction heuristic followed by intra-route 2-opt.

use crate::domain::{ProblemData, VrpPlan};

pub fn run(plan: &mut VrpPlan) {
    let n_vehicles = plan.vehicles.len();
    if n_vehicles == 0 {
        return;
    }
    let data = unsafe { &*(plan.vehicles[0].data as *const ProblemData) };
    let n_nodes = data.demands.len();
    if n_nodes <= 1 {
        return;
    }

    let depot = data.depot;
    let capacity = data.capacity;

    // 1. Compute savings s(i,j) = d(depot,i) + d(depot,j) - d(i,j)
    let mut savings: Vec<(i64, usize, usize)> = Vec::new();
    for i in 1..n_nodes {
        if i == depot {
            continue;
        }
        for j in (i + 1)..n_nodes {
            if j == depot {
                continue;
            }
            let s = data.distance_matrix[depot][i]
                + data.distance_matrix[depot][j]
                - data.distance_matrix[i][j];
            savings.push((s, i, j));
        }
    }
    savings.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    // 2. Start with each customer in its own singleton route
    let mut routes: Vec<Vec<usize>> = (1..n_nodes).filter(|&c| c != depot).map(|c| vec![c]).collect();
    let mut route_load: Vec<i64> = routes.iter().map(|r| data.demands[r[0]] as i64).collect();
    let mut route_of: Vec<Option<usize>> = vec![None; n_nodes];
    for (ri, r) in routes.iter().enumerate() {
        route_of[r[0]] = Some(ri);
    }

    // 3. Merge greedily
    for (_, i, j) in savings {
        let ri = match route_of[i] { Some(r) => r, None => continue };
        let rj = match route_of[j] { Some(r) => r, None => continue };
        if ri == rj { continue; }
        if route_load[ri] + route_load[rj] > capacity { continue; }

        let i_at_start = routes[ri].first() == Some(&i);
        let i_at_end   = routes[ri].last()  == Some(&i);
        let j_at_start = routes[rj].first() == Some(&j);
        let j_at_end   = routes[rj].last()  == Some(&j);
        if !i_at_end && !i_at_start { continue; }
        if !j_at_end && !j_at_start { continue; }

        if i_at_start { routes[ri].reverse(); }
        if j_at_end   { routes[rj].reverse(); }

        let rj_customers: Vec<usize> = routes[rj].drain(..).collect();
        route_load[ri] += route_load[rj];
        route_load[rj] = 0;
        for &c in &rj_customers { route_of[c] = Some(ri); }
        routes[ri].extend(rj_customers);
    }

    // 4. Assign non-empty routes to vehicles
    let mut vid = 0;
    for route in routes {
        if route.is_empty() { continue; }
        if vid >= n_vehicles { break; }
        plan.vehicles[vid].visits = route;
        vid += 1;
    }

    // 5. Intra-route 2-opt
    for vehicle in &mut plan.vehicles {
        two_opt(&mut vehicle.visits, data);
    }
}

fn two_opt(route: &mut Vec<usize>, data: &ProblemData) {
    let n = route.len();
    if n < 4 { return; }
    let d = data.depot;
    let dm = &data.distance_matrix;
    loop {
        let mut improved = false;
        for i in 0..n - 1 {
            let a = if i == 0 { d } else { route[i - 1] };
            let b = route[i];
            for j in i + 1..n {
                let c = route[j];
                let e = if j + 1 < n { route[j + 1] } else { d };
                if dm[a][c] + dm[b][e] < dm[a][b] + dm[c][e] {
                    route[i..=j].reverse();
                    improved = true;
                }
            }
        }
        if !improved { break; }
    }
}
