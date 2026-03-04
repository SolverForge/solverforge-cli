/// Shared problem data — capacity, depot index, demands, and the distance matrix.
///
/// Stored once and shared with all vehicles via a raw pointer to avoid
/// copying the matrix for every entity. The `Box` that owns this struct
/// must outlive the `VrpPlan`.
pub struct ProblemData {
    pub capacity: i64,
    pub depot: usize,
    pub demands: Vec<i32>,
    /// distance_matrix[i][j] — integer distance between nodes i and j.
    pub distance_matrix: Vec<Vec<i64>>,
}

impl ProblemData {
    /// Total route distance for a given sequence of visits (depot → visits → depot).
    pub fn route_distance(&self, visits: &[usize]) -> i64 {
        if visits.is_empty() {
            return 0;
        }
        let mut dist = self.distance_matrix[self.depot][visits[0]];
        for w in visits.windows(2) {
            dist += self.distance_matrix[w[0]][w[1]];
        }
        dist += self.distance_matrix[*visits.last().unwrap()][self.depot];
        dist
    }

    /// Total demand for a given sequence of visits.
    pub fn route_demand(&self, visits: &[usize]) -> i64 {
        visits.iter().map(|&i| self.demands[i] as i64).sum()
    }
}
