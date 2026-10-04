//! Cross-platform ComfyUI node & model installer.
//!
//! Replaces the Windows-only batch file (`watertightMeshes_win_installer.bat`)
//! with a robust, platform-agnostic installer supporting Linux, macOS, and Windows.

use crate::error::{Result, WatertightError};
use crate::workflows;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned commit reference for reproducibility
pub const PIN_DATE: &str = "2026-09-26";

/// A ComfyUI Custom Node repository
#[derive(Debug, Clone)]
pub struct CustomNodePack {
    pub name: &'static str,
    pub repo_url: &'static str,
    pub pinned_sha: &'static str,
    pub description: &'static str,
}

/// Official list of 9 MostAadTech node packs tested and curated by PixelArtistry
pub const NODE_PACKS: &[CustomNodePack] = &[
    CustomNodePack {
        name: "WTiVo-WatertightVoxel-ComfyuiNode",
        repo_url: "https://github.com/Mstafa-awad/WTiVo-WatertightVoxel-ComfyuiNode.git",
        pinned_sha: "fb9e9ea6a7deec965d3a4bf43ba7a4a9ff3856f3",
        description: "Watertight Voxelization core node",
    },
    CustomNodePack {
        name: "ComfyUI-Mesh-Quad-Reconstruct",
        repo_url: "https://github.com/Mstafa-awad/ComfyUI-Mesh-Quad-Reconstruct.git",
        pinned_sha: "74048b3415537e26cc72b46b082a010cd0e357ce",
        description: "Quad topology mesh reconstruction",
    },
    CustomNodePack {
        name: "ComfyUI-CuMesh-Decimate",
        repo_url: "https://github.com/Mstafa-awad/ComfyUI-CuMesh-Decimate.git",
        pinned_sha: "c4edeb96bf637239cecb731d2869469a3025d742",
        description: "CUDA-accelerated mesh decimation",
    },
    CustomNodePack {
        name: "ComfyUI-Memory-Cleaner",
        repo_url: "https://github.com/Mstafa-awad/ComfyUI-Memory-Cleaner.git",
        pinned_sha: "3357282290278c96ffa0da180d43bc5eac5f2286",
        description: "VRAM and system memory cleanup utility",
    },
    CustomNodePack {
        name: "LODTailor-The-Mesh-Trimmer-ComfyuiNode",
        repo_url: "https://github.com/Mstafa-awad/LODTailor-The-Mesh-Trimmer-ComfyuiNode.git",
        pinned_sha: "3d25b7d4aa382fa5dac210eb5d8d0eadc4a4f183",
        description: "LOD tailoring and geometry trimmer",
    },
    CustomNodePack {
        name: "LODTailor-Bake-Forger",
        repo_url: "https://github.com/Mstafa-awad/LODTailor-Bake-Forger.git",
        pinned_sha: "f13589c22558e3ca49dfa87dce709233eb3cc86b",
        description: "Texture and normal map bake forger (Blender)",
    },
    CustomNodePack {
        name: "Trellis2-Mesh-Encoder",
        repo_url: "https://github.com/Mstafa-awad/Trellis2-Mesh-Encoder.git",
        pinned_sha: "860313e67f0f69d402310dc325707bab8bec2fe9",
        description: "TRELLIS.2 mesh feature encoder",
    },
    CustomNodePack {
        name: "ComfyUI_LODsmith_Merge_Watertight",
        repo_url: "https://github.com/Mstafa-awad/ComfyUI_LODsmith_Merge_Watertight.git",
        pinned_sha: "5da6dd69c02d0f3d4ae743d090cfac79aefe7f96",
        description: "LODsmith watertight mesh merger",
    },
    CustomNodePack {
        name: "WTiVo-FastMergeByDistance",
        repo_url: "https://github.com/Mstafa-awad/WTiVo-FastMergeByDistance.git",
        pinned_sha: "5392949165ceed1e74448b1949d6e0dc9b34dd92",
        description: "Accelerated vertex distance welding",
    },
];

/// A Hugging Face model checkpoint required by the workflows
#[derive(Debug, Clone)]
pub struct ModelFile {
    pub subfolder: &'static str,
    pub filename: &'static str,
    pub download_url: &'static str,
}

pub const MODEL_FILES: &[ModelFile] = &[
    ModelFile {
        subfolder: "diffusion_models",
        filename: "trellis_2_int8_convrot.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/TRELLIS.2/resolve/main/diffusion_models/trellis_2_int8_convrot.safetensors",
    },
    ModelFile {
        subfolder: "diffusion_models",
        filename: "pixal3d_int8_convrot.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/Pixal3D/resolve/main/diffusion_models/pixal3d_int8_convrot.safetensors",
    },
    ModelFile {
        subfolder: "vae",
        filename: "trellis_2_shape_vae_bf16.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/Pixal3D/resolve/main/vae/trellis_2_shape_vae_bf16.safetensors",
    },
    ModelFile {
        subfolder: "vae",
        filename: "trellis_2_texture_vae_bf16.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/Pixal3D/resolve/main/vae/trellis_2_texture_vae_bf16.safetensors",
    },
    ModelFile {
        subfolder: "clip_vision",
        filename: "dino_v3_L_naf_fp32.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/Pixal3D/resolve/main/clip_vision/dino_v3_L_naf_fp32.safetensors",
    },
    ModelFile {
        subfolder: "geometry_estimation",
        filename: "moge_2_vitl_normal_fp16.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/MoGe/resolve/main/geometry_estimation/moge_2_vitl_normal_fp16.safetensors",
    },
    ModelFile {
        subfolder: "background_removal",
        filename: "birefnet.safetensors",
        download_url: "https://huggingface.co/Comfy-Org/BiRefNet/resolve/main/background_removal/birefnet.safetensors",
    },
    ModelFile {
        subfolder: "Trellis2/encoders",
        filename: "shape_enc_next_dc_f16c32_fp16.safetensors",
        download_url: "https://huggingface.co/microsoft/TRELLIS.2-4B/resolve/main/ckpts/shape_enc_next_dc_f16c32_fp16.safetensors",
    },
    ModelFile {
        subfolder: "Trellis2/encoders",
        filename: "shape_enc_next_dc_f16c32_fp16.json",
        download_url: "https://huggingface.co/microsoft/TRELLIS.2-4B/resolve/main/ckpts/shape_enc_next_dc_f16c32_fp16.json",
    },
];

/// Try to locate the ComfyUI installation directory
pub fn detect_comfyui_dir(hint: Option<&Path>) -> Option<PathBuf> {
    if let Some(h) = hint {
        if h.is_dir() && (h.join("custom_nodes").exists() || h.join("models").exists() || h.join("main.py").exists()) {
            return Some(h.to_path_buf());
        }
    }

    let candidates = [
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../ComfyUI"),
        PathBuf::from("ComfyUI"),
        dirs_home_dir().map(|h| h.join("ComfyUI")).unwrap_or_default(),
        dirs_home_dir().map(|h| h.join("Documents/ComfyUI")).unwrap_or_default(),
        dirs_home_dir().map(|h| h.join("ai/ComfyUI")).unwrap_or_default(),
    ];

    for c in &candidates {
        if c.is_dir() && c.join("custom_nodes").exists() {
            return fs::canonicalize(c).ok();
        }
    }

    None
}

fn dirs_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Status of ComfyUI installation and nodes
#[derive(Debug)]
pub struct InstallationReport {
    pub comfy_dir: PathBuf,
    pub nodes_present: usize,
    pub nodes_missing: usize,
    pub models_present: usize,
    pub models_missing: usize,
    pub workflows_installed: bool,
}

/// Audit custom nodes and models inside a ComfyUI directory
pub fn audit_comfyui(comfy_dir: &Path) -> InstallationReport {
    let nodes_dir = comfy_dir.join("custom_nodes");
    let models_dir = comfy_dir.join("models");

    let mut nodes_present = 0;
    let mut nodes_missing = 0;
    for node in NODE_PACKS {
        let path = nodes_dir.join(node.name);
        if path.exists() {
            nodes_present += 1;
        } else {
            nodes_missing += 1;
        }
    }

    let mut models_present = 0;
    let mut models_missing = 0;
    for m in MODEL_FILES {
        let p = models_dir.join(m.subfolder).join(m.filename);
        if p.exists() {
            models_present += 1;
        } else {
            models_missing += 1;
        }
    }

    let wf_dir = comfy_dir.join("user/default/workflows/PixelArtistry");
    let workflows_installed = wf_dir.join("PixelArtistry_01_Image_to_Watertight_Mesh.json").exists();

    InstallationReport {
        comfy_dir: comfy_dir.to_path_buf(),
        nodes_present,
        nodes_missing,
        models_present,
        models_missing,
        workflows_installed,
    }
}

/// Install custom node packs into ComfyUI/custom_nodes
pub fn install_node_packs(comfy_dir: &Path, use_pinned: bool) -> Result<()> {
    let nodes_dir = comfy_dir.join("custom_nodes");
    fs::create_dir_all(&nodes_dir)?;

    println!(
        "{} Installing {} MostAadTech node packs into {}",
        "==>".bold().green(),
        NODE_PACKS.len(),
        nodes_dir.display()
    );

    for node in NODE_PACKS {
        let target = nodes_dir.join(node.name);
        if target.exists() {
            println!(
                "  {} [{}] already installed",
                "✓".green(),
                node.name.bold()
            );
            if use_pinned {
                // Checkout pinned commit
                let _ = Command::new("git")
                    .current_dir(&target)
                    .args(["checkout", node.pinned_sha])
                    .output();
            }
        } else {
            print!("  {} Cloning {}... ", "⬇".cyan(), node.name.bold());
            std::io::stdout().flush().ok();

            let status = Command::new("git")
                .args(["clone", node.repo_url, target.to_str().unwrap()])
                .output();

            match status {
                Ok(out) if out.status.success() => {
                    println!("{}", "done".green());
                    if use_pinned {
                        let _ = Command::new("git")
                            .current_dir(&target)
                            .args(["checkout", node.pinned_sha])
                            .output();
                    }
                }
                _ => {
                    println!("{}", "failed (git not available or connection error)".red());
                }
            }
        }
    }

    Ok(())
}

/// Install authentic PixelArtistry ComfyUI workflows and test assets
pub fn install_workflows(comfy_dir: &Path) -> Result<()> {
    let wf_dir = comfy_dir.join("user").join("default").join("workflows").join("PixelArtistry");
    fs::create_dir_all(&wf_dir)?;

    println!(
        "{} Deploying PixelArtistry workflows to {}",
        "==>".bold().green(),
        wf_dir.display()
    );

    let count = workflows::export_all_workflows(&wf_dir)?;
    println!(
        "  {} Installed {} workflows to ComfyUI sidebar directory",
        "✓".green(),
        count
    );

    // Download default test input if missing
    let input_dir = comfy_dir.join("input");
    fs::create_dir_all(&input_dir)?;
    let sample_img = input_dir.join("viking_wolf_rune_axe.png");
    if !sample_img.exists() {
        print!("  {} Fetching sample image (viking_wolf_rune_axe.png)... ", "⬇".cyan());
        std::io::stdout().flush().ok();
        let url = "https://raw.githubusercontent.com/Comfy-Org/workflow_templates/refs/heads/main/input/viking_wolf_rune_axe.png";
        if let Ok(mut resp) = ureq::get(url).call() {
            let mut bytes = Vec::new();
            if resp.body_mut().as_reader().read_to_end(&mut bytes).is_ok() {
                let mut f = File::create(&sample_img)?;
                f.write_all(&bytes)?;
                println!("{}", "done".green());
            } else {
                println!("{}", "skip".yellow());
            }
        } else {
            println!("{}", "skip".yellow());
        }
    }

    Ok(())
}

/// Download a model checkpoint with progress bar
pub fn download_model(target_dir: &Path, file: &ModelFile) -> Result<()> {
    let out_dir = target_dir.join(file.subfolder);
    fs::create_dir_all(&out_dir)?;

    let out_path = out_dir.join(file.filename);
    if out_path.exists() {
        println!("  {} {} already exists", "✓".green(), file.filename);
        return Ok(());
    }

    println!("  {} Downloading {}", "⬇".cyan(), file.filename.bold());
    println!("     URL: {}", file.download_url.dimmed());

    let mut resp = ureq::get(file.download_url)
        .call()
        .map_err(|e| WatertightError::Io(std::io::Error::other(e.to_string())))?;

    let content_len = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let pb = ProgressBar::new(content_len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let part_path = out_dir.join(format!("{}.part", file.filename));
    let mut out_file = File::create(&part_path)?;
    let mut reader = resp.body_mut().as_reader();
    let mut buffer = [0u8; 65536];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        out_file.write_all(&buffer[..n])?;
        pb.inc(n as u64);
    }

    pb.finish_with_message("downloaded");
    fs::rename(part_path, out_path)?;

    Ok(())
}
