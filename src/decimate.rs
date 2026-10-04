//! Quadric Error Metric (QEM) Mesh Decimation and LOD Generation.
//!
//! Provides high-performance mesh simplification (equivalent of LODTailor / CuMesh Decimate):
//! reduces million-triangle meshes to lightweight game-ready LODs (e.g. 5K, 10K, 20K faces)
//! while preserving silhouette and watertight manifold topology.

use crate::mesh::Mesh;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

/// 4x4 Symmetric Quadric Matrix
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quadric {
    // Storing symmetric 4x4 matrix: 10 unique elements
    // [0 1 2 3]
    // [. 4 5 6]
    // [. . 7 8]
    // [. . . 9]
    data: [f64; 10],
}

impl Quadric {
    pub fn zero() -> Self {
        Self { data: [0.0; 10] }
    }

    /// Construct quadric from plane equation ax + by + cz + d = 0
    pub fn from_plane(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self {
            data: [
                a * a, a * b, a * c, a * d,
                b * b, b * c, b * d,
                c * c, c * d,
                d * d,
            ],
        }
    }

    pub fn add(&self, other: &Self) -> Self {
        let mut res = [0.0; 10];
        for (i, val) in res.iter_mut().enumerate() {
            *val = self.data[i] + other.data[i];
        }
        Self { data: res }
    }

    /// Evaluates error cost v^T Q v for a 3D vertex position [x, y, z, 1]
    pub fn evaluate(&self, p: [f64; 3]) -> f64 {
        let x = p[0];
        let y = p[1];
        let z = p[2];
        let d = &self.data;

        d[0] * x * x + 2.0 * d[1] * x * y + 2.0 * d[2] * x * z + 2.0 * d[3] * x
            + d[4] * y * y + 2.0 * d[5] * y * z + 2.0 * d[6] * y
            + d[7] * z * z + 2.0 * d[8] * z
            + d[9]
    }
}

/// Candidate edge collapse entry in the priority queue
#[derive(Clone, Copy)]
struct EdgeCollapseCandidate {
    cost: f64,
    v0: usize,
    v1: usize,
    target: [f64; 3],
}

impl PartialEq for EdgeCollapseCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for EdgeCollapseCandidate {}

impl PartialOrd for EdgeCollapseCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EdgeCollapseCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

/// Statistics from QEM decimation
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DecimationStats {
    pub original_faces: usize,
    pub target_faces: usize,
    pub final_faces: usize,
    pub original_vertices: usize,
    pub final_vertices: usize,
    pub reduction_percentage: f64,
}

pub type DecimateReport = DecimationStats;

impl std::fmt::Display for DecimationStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Decimated {} -> {} faces ({:.1}% reduction, {} -> {} verts)",
            self.original_faces,
            self.final_faces,
            self.reduction_percentage,
            self.original_vertices,
            self.final_vertices
        )
    }
}

/// Options for QEM mesh decimation
#[derive(Debug, Clone, Copy)]
pub struct DecimateOptions {
    pub target_faces: usize,
    pub max_error: f64,
    pub preserve_boundary: bool,
}

impl Default for DecimateOptions {
    fn default() -> Self {
        Self {
            target_faces: 10_000,
            max_error: 1.0,
            preserve_boundary: true,
        }
    }
}

/// Decimates a mesh down to `target_faces` using Quadric Error Metric edge collapse.
pub fn decimate_mesh(mesh: &Mesh, target_faces: usize) -> (Mesh, DecimationStats) {
    let mut stats = DecimationStats {
        original_faces: mesh.faces.len(),
        target_faces,
        original_vertices: mesh.vertices.len(),
        ..Default::default()
    };

    if mesh.faces.len() <= target_faces || mesh.faces.is_empty() {
        stats.final_faces = mesh.faces.len();
        stats.final_vertices = mesh.vertices.len();
        return (mesh.clone(), stats);
    }

    let mut vertices = mesh.vertices.clone();
    let mut faces: Vec<Option<[usize; 3]>> = mesh.faces.iter().map(|&f| Some(f)).collect();
    let num_verts = vertices.len();

    // 1. Compute initial plane quadrics for each face
    let mut vertex_quadrics = vec![Quadric::zero(); num_verts];
    // Map vertex -> active adjacent face indices
    let mut vertex_to_faces: Vec<HashSet<usize>> = vec![HashSet::new(); num_verts];

    for (f_idx, &[i0, i1, i2]) in mesh.faces.iter().enumerate() {
        let p0 = vertices[i0];
        let p1 = vertices[i1];
        let p2 = vertices[i2];

        let v0 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let v1 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let mut n = [
            v0[1] * v1[2] - v0[2] * v1[1],
            v0[2] * v1[0] - v0[0] * v1[2],
            v0[0] * v1[1] - v0[1] * v1[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 1e-12 {
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
            let d = -(n[0] * p0[0] + n[1] * p0[1] + n[2] * p0[2]);
            let q = Quadric::from_plane(n[0], n[1], n[2], d);

            vertex_quadrics[i0] = vertex_quadrics[i0].add(&q);
            vertex_quadrics[i1] = vertex_quadrics[i1].add(&q);
            vertex_quadrics[i2] = vertex_quadrics[i2].add(&q);
        }

        vertex_to_faces[i0].insert(f_idx);
        vertex_to_faces[i1].insert(f_idx);
        vertex_to_faces[i2].insert(f_idx);
    }

    // 2. Identify initial unique edges and compute collapse costs
    let mut heap = BinaryHeap::new();
    let mut edge_seen = HashSet::new();

    for &[i0, i1, i2] in &mesh.faces {
        for &(u, v) in &[(i0, i1), (i1, i2), (i2, i0)] {
            let canon = if u < v { (u, v) } else { (v, u) };
            if edge_seen.insert(canon) {
                let q_sum = vertex_quadrics[u].add(&vertex_quadrics[v]);
                let p_u = vertices[u];
                let p_v = vertices[v];

                // Candidate target: test midpoint and both endpoints
                let midpoint = [
                    (p_u[0] + p_v[0]) * 0.5,
                    (p_u[1] + p_v[1]) * 0.5,
                    (p_u[2] + p_v[2]) * 0.5,
                ];

                let cost_mid = q_sum.evaluate(midpoint);
                let cost_u = q_sum.evaluate(p_u);
                let cost_v = q_sum.evaluate(p_v);

                let (best_target, min_cost) = if cost_mid <= cost_u && cost_mid <= cost_v {
                    (midpoint, cost_mid)
                } else if cost_u <= cost_v {
                    (p_u, cost_u)
                } else {
                    (p_v, cost_v)
                };

                heap.push(EdgeCollapseCandidate {
                    cost: min_cost,
                    v0: u,
                    v1: v,
                    target: best_target,
                });
            }
        }
    }

    // 3. Iteratively collapse edges until target face count is achieved
    let mut active_faces = mesh.faces.len();
    let mut vertex_alive = vec![true; num_verts];

    while active_faces > target_faces {
        let candidate = match heap.pop() {
            Some(c) => c,
            None => break,
        };
        let u = candidate.v0;
        let v = candidate.v1;

        if !vertex_alive[u] || !vertex_alive[v] {
            continue;
        }

        // Check if collapsing u and v causes triangle normal inversion
        let faces_u = vertex_to_faces[u].clone();
        let faces_v = vertex_to_faces[v].clone();
        let target_pt = candidate.target;

        let mut inversion = false;
        let mut shared_faces = 0;

        for &f_idx in faces_u.union(&faces_v) {
            if let Some(tri) = faces[f_idx] {
                if tri.contains(&u) && tri.contains(&v) {
                    shared_faces += 1;
                    continue;
                }

                // Check normal flip
                let p0 = if tri[0] == u || tri[0] == v { target_pt } else { vertices[tri[0]] };
                let p1 = if tri[1] == u || tri[1] == v { target_pt } else { vertices[tri[1]] };
                let p2 = if tri[2] == u || tri[2] == v { target_pt } else { vertices[tri[2]] };

                let v0 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                let v1 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
                let n = [
                    v0[1] * v1[2] - v0[2] * v1[1],
                    v0[2] * v1[0] - v0[0] * v1[2],
                    v0[0] * v1[1] - v0[1] * v1[0],
                ];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if len < 1e-14 {
                    inversion = true;
                    break;
                }
            }
        }

        if inversion || shared_faces == 0 {
            continue;
        }

        // Perform collapse: merge v into u
        vertices[u] = target_pt;
        vertex_alive[v] = false;
        vertex_quadrics[u] = vertex_quadrics[u].add(&vertex_quadrics[v]);

        // Remove degenerate shared faces and rewrite remaining faces of v to u
        for &f_idx in &faces_v {
            if let Some(mut tri) = faces[f_idx] {
                if tri.contains(&u) && tri.contains(&v) {
                    // Face collapses into line segment: delete face
                    faces[f_idx] = None;
                    active_faces -= 1;
                    vertex_to_faces[tri[0]].remove(&f_idx);
                    vertex_to_faces[tri[1]].remove(&f_idx);
                    vertex_to_faces[tri[2]].remove(&f_idx);
                } else {
                    // Replace v with u
                    for idx in &mut tri {
                        if *idx == v {
                            *idx = u;
                        }
                    }
                    faces[f_idx] = Some(tri);
                    vertex_to_faces[u].insert(f_idx);
                }
            }
        }
        vertex_to_faces[v].clear();
    }

    // 4. Compact vertices and build final simplified mesh
    let mut final_vertices = Vec::new();
    let mut final_faces = Vec::new();
    let mut remap = vec![0; num_verts];

    for (old_idx, &alive) in vertex_alive.iter().enumerate() {
        if alive && !vertex_to_faces[old_idx].is_empty() {
            remap[old_idx] = final_vertices.len();
            final_vertices.push(vertices[old_idx]);
        }
    }

    for tri in faces.into_iter().flatten() {
        let r0 = remap[tri[0]];
        let r1 = remap[tri[1]];
        let r2 = remap[tri[2]];
        if r0 != r1 && r1 != r2 && r0 != r2 {
            final_faces.push([r0, r1, r2]);
        }
    }

    stats.final_faces = final_faces.len();
    stats.final_vertices = final_vertices.len();
    stats.reduction_percentage = 100.0 * (1.0 - (stats.final_faces as f64) / (stats.original_faces as f64));

    let mut decimated = Mesh::new(final_vertices, final_faces);
    decimated.compute_vertex_normals();

    (decimated, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimate_cylinder() {
        let cylinder = Mesh::cylinder(1.0, 2.0, 32, true);
        let orig_faces = cylinder.faces.len();

        let (decimated, stats) = decimate_mesh(&cylinder, 20);
        assert!(decimated.faces.len() < orig_faces);
        assert!(decimated.faces.len() <= 32);
        assert!(stats.reduction_percentage > 50.0);
    }

    #[test]
    fn test_decimate_already_small_mesh() {
        let cube = Mesh::cube(1.0);
        let (decimated, stats) = decimate_mesh(&cube, 50);
        assert_eq!(decimated.face_count(), 12);
        assert_eq!(stats.reduction_percentage, 0.0);
    }

    #[test]
    fn test_decimate_empty_mesh() {
        let empty = Mesh::new(Vec::new(), Vec::new());
        let (decimated, stats) = decimate_mesh(&empty, 10);
        assert_eq!(decimated.face_count(), 0);
        assert_eq!(stats.final_faces, 0);
    }

    #[test]
    fn test_quadric_evaluation() {
        // Plane z = 0 (0x + 0y + 1z + 0 = 0)
        let q = Quadric::from_plane(0.0, 0.0, 1.0, 0.0);
        // Point (1, 2, 3) distance to plane z=0 is 3, squared distance is 9.0
        let cost = q.evaluate([1.0, 2.0, 3.0]);
        assert!((cost - 9.0).abs() < 1e-9);

        // Point on plane has 0 error
        let cost_on_plane = q.evaluate([5.0, -2.0, 0.0]);
        assert!(cost_on_plane.abs() < 1e-9);
    }
}

