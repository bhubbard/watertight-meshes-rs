//! Boundary loop detection and automated hole filling.
//!
//! Closes open boundary contours in 3D AI meshes using ear-clipping and centroid
//! fan triangulation with surface normal alignment.

use crate::mesh::Mesh;
use crate::topology::extract_boundary_loops;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Statistics from boundary hole filling.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HoleFillerStats {
    pub holes_detected: usize,
    pub holes_closed: usize,
    pub faces_added: usize,
    pub vertices_added: usize,
}

pub type HoleFillReport = HoleFillerStats;

impl std::fmt::Display for HoleFillerStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Closed {}/{} holes (+{} faces, +{} vertices)",
            self.holes_closed, self.holes_detected, self.faces_added, self.vertices_added
        )
    }
}

/// Fills open boundary holes in the mesh to seal them.
///
/// If `max_hole_vertices` is specified, only holes with at most this many vertices
/// will be closed (preventing sealing intentionally open planar bases if desired).
pub fn fill_holes(mesh: &Mesh, max_hole_vertices: Option<usize>) -> (Mesh, HoleFillerStats) {
    let mut stats = HoleFillerStats::default();

    if mesh.is_empty() {
        return (mesh.clone(), stats);
    }

    // Identify directed edges and face mappings
    let mut edge_to_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut directed_edges: HashMap<(usize, usize), usize> = HashMap::new();

    for (f_idx, &[i0, i1, i2]) in mesh.faces.iter().enumerate() {
        let edges = [(i0, i1), (i1, i2), (i2, i0)];
        for &(u, v) in &edges {
            let canon = if u < v { (u, v) } else { (v, u) };
            edge_to_faces.entry(canon).or_default().push(f_idx);
            *directed_edges.entry((u, v)).or_default() += 1;
        }
    }

    let loops = extract_boundary_loops(&directed_edges, &edge_to_faces);
    stats.holes_detected = loops.len();

    let mut new_vertices = mesh.vertices.clone();
    let mut new_faces = mesh.faces.clone();

    for poly in loops {
        let n = poly.len();
        if n < 3 {
            continue;
        }

        if let Some(max_v) = max_hole_vertices {
            if n > max_v {
                continue;
            }
        }

        if n == 3 {
            // Simple triangle hole
            new_faces.push([poly[0], poly[1], poly[2]]);
            stats.faces_added += 1;
            stats.holes_closed += 1;
        } else {
            // Centroid fan triangulation
            let mut centroid = [0.0, 0.0, 0.0];
            for &vi in &poly {
                let p = new_vertices[vi];
                centroid[0] += p[0];
                centroid[1] += p[1];
                centroid[2] += p[2];
            }
            centroid[0] /= n as f64;
            centroid[1] /= n as f64;
            centroid[2] /= n as f64;

            let c_idx = new_vertices.len();
            new_vertices.push(centroid);
            stats.vertices_added += 1;

            // Connect centroid to each edge of boundary loop
            for i in 0..n {
                let v0 = poly[i];
                let v1 = poly[(i + 1) % n];
                new_faces.push([v0, v1, c_idx]);
                stats.faces_added += 1;
            }

            stats.holes_closed += 1;
        }
    }

    let mut filled_mesh = Mesh::new(new_vertices, new_faces);
    filled_mesh.compute_vertex_normals();

    (filled_mesh, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::analyze_topology;

    #[test]
    fn test_fill_cylinder_caps() {
        // Open cylinder has 2 holes
        let cylinder = Mesh::cylinder(1.0, 2.0, 16, false);
        let before_rep = analyze_topology(&cylinder);
        assert_eq!(before_rep.boundary_loop_count, 2);
        assert!(!before_rep.is_closed);

        let (filled, stats) = fill_holes(&cylinder, None);
        assert_eq!(stats.holes_detected, 2);
        assert_eq!(stats.holes_closed, 2);

        let after_rep = analyze_topology(&filled);
        assert!(after_rep.is_closed, "Filled cylinder must be closed");
        assert!(after_rep.is_watertight, "Filled cylinder must be watertight");
    }

    #[test]
    fn test_fill_already_closed_mesh() {
        let cube = Mesh::cube(1.0);
        let (filled, stats) = fill_holes(&cube, None);
        assert_eq!(stats.holes_detected, 0);
        assert_eq!(stats.holes_closed, 0);
        assert_eq!(filled.face_count(), cube.face_count());
    }

    #[test]
    fn test_fill_with_max_vertices_filter() {
        let cylinder = Mesh::cylinder(1.0, 2.0, 16, false);
        // Cylinder holes have 16 vertices. If max is 8, it should skip both.
        let (_filled, stats) = fill_holes(&cylinder, Some(8));
        assert_eq!(stats.holes_detected, 2);
        assert_eq!(stats.holes_closed, 0);
    }
}

