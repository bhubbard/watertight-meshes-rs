//! Embedded PixelArtistry ComfyUI workflows and export manager.

use crate::error::{Result, WatertightError};
use std::fs;
use std::path::{Path, PathBuf};

pub const WORKFLOW_01_NAME: &str = "PixelArtistry_01_Image_to_Watertight_Mesh.json";
pub const WORKFLOW_01B_NAME: &str = "PixelArtistry_01b_Image_to_Watertight_Mesh_2K.json";
pub const WORKFLOW_02_NAME: &str = "PixelArtistry_02_Image_to_GameReady_Asset.json";
pub const WORKFLOW_03_NAME: &str = "PixelArtistry_03_Your_Mesh_to_GameReady_Asset.json";

pub const WORKFLOW_01_CONTENT: &str =
    include_str!("../workflows/PixelArtistry_01_Image_to_Watertight_Mesh.json");
pub const WORKFLOW_01B_CONTENT: &str =
    include_str!("../workflows/PixelArtistry_01b_Image_to_Watertight_Mesh_2K.json");
pub const WORKFLOW_02_CONTENT: &str =
    include_str!("../workflows/PixelArtistry_02_Image_to_GameReady_Asset.json");
pub const WORKFLOW_03_CONTENT: &str =
    include_str!("../workflows/PixelArtistry_03_Your_Mesh_to_GameReady_Asset.json");

/// Metadata for an embedded workflow
#[derive(Debug, Clone)]
pub struct WorkflowMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub target_use_case: &'static str,
    pub json_content: &'static str,
}

/// Returns the metadata for all embedded PixelArtistry workflows.
pub fn get_all_workflows() -> Vec<WorkflowMeta> {
    vec![
        WorkflowMeta {
            name: WORKFLOW_01_NAME,
            description: "Image → Watertight 3D Mesh (1536 resolution, 3D print ready)",
            target_use_case: "3D Printing & Watertight solid generation from reference images",
            json_content: WORKFLOW_01_CONTENT,
        },
        WorkflowMeta {
            name: WORKFLOW_01B_NAME,
            description: "Image → Watertight 3D Mesh 2K (2048 high-res, 16GB+ VRAM)",
            target_use_case: "Ultra-high-detail 3D printing on RTX 4090/5080/5090 or Apple Silicon",
            json_content: WORKFLOW_01B_CONTENT,
        },
        WorkflowMeta {
            name: WORKFLOW_02_NAME,
            description: "Image → Textured High-Poly + Baked Game-Ready Low-Poly (LOD)",
            target_use_case: "Complete AAA game-asset pipeline with LODTailor + Blender baking",
            json_content: WORKFLOW_02_CONTENT,
        },
        WorkflowMeta {
            name: WORKFLOW_03_NAME,
            description: "Your Own Mesh → Textured + Baked Game-Ready Low-Poly (LOD)",
            target_use_case: "Take any external raw/messy AI mesh and produce clean game-ready LOD",
            json_content: WORKFLOW_03_CONTENT,
        },
    ]
}

/// Legacy helper for available workflows
pub fn available_workflows() -> Vec<(&'static str, &'static str, &'static str)> {
    get_all_workflows()
        .into_iter()
        .map(|w| (w.name, w.description, w.json_content))
        .collect()
}

/// Retrieves the raw JSON string for a specific workflow by filename.
pub fn get_workflow(name: &str) -> Option<&'static str> {
    for wf in get_all_workflows() {
        if wf.name == name || wf.name.trim_end_matches(".json") == name.trim_end_matches(".json") {
            return Some(wf.json_content);
        }
    }
    None
}

/// Exports all embedded workflows to a target directory.
pub fn export_workflows<P: AsRef<Path>>(target_dir: P) -> Result<Vec<PathBuf>> {
    let dir = target_dir.as_ref();
    fs::create_dir_all(dir)?;

    let mut written = Vec::new();
    for wf in get_all_workflows() {
        let dest = dir.join(wf.name);
        fs::write(&dest, wf.json_content)?;
        written.push(dest);
    }

    Ok(written)
}

/// Exports all workflows returning the count written
pub fn export_all_workflows<P: AsRef<Path>>(target_dir: P) -> Result<usize> {
    let list = export_workflows(target_dir)?;
    Ok(list.len())
}

/// Validates that an embedded workflow is valid JSON.
pub fn validate_workflow(name: &str) -> Result<serde_json::Value> {
    let content = get_workflow(name).ok_or_else(|| {
        WatertightError::WorkflowError(format!("Workflow '{}' not found", name))
    })?;
    let val: serde_json::Value = serde_json::from_str(content)?;
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_embedded_workflows_are_valid_json() {
        for (name, _, content) in available_workflows() {
            let res: std::result::Result<serde_json::Value, _> = serde_json::from_str(content);
            assert!(
                res.is_ok(),
                "Workflow {} must parse as valid JSON: {:?}",
                name,
                res.err()
            );
        }
    }

    #[test]
    fn test_get_workflow_lookup() {
        let wf1 = get_workflow("PixelArtistry_01_Image_to_Watertight_Mesh.json");
        assert!(wf1.is_some());
        let wf1_no_ext = get_workflow("PixelArtistry_01_Image_to_Watertight_Mesh");
        assert!(wf1_no_ext.is_some());
        assert_eq!(wf1, wf1_no_ext);

        let missing = get_workflow("non_existent_workflow.json");
        assert!(missing.is_none());
    }

    #[test]
    fn test_export_all_workflows_to_disk() {
        let temp_dir = std::env::temp_dir().join(format!("test_wf_export_{}", std::process::id()));
        let count = export_all_workflows(&temp_dir).expect("export workflows failed");
        assert_eq!(count, 4);

        for wf in get_all_workflows() {
            let p = temp_dir.join(wf.name);
            assert!(p.exists());
            let read = fs::read_to_string(&p).unwrap();
            assert_eq!(read, wf.json_content);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_validate_workflow() {
        assert!(validate_workflow("PixelArtistry_01_Image_to_Watertight_Mesh.json").is_ok());
        assert!(validate_workflow("fake_invalid").is_err());
    }
}

