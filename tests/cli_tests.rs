//! Integration tests for the Watertight Meshes CLI binary.

use std::fs;
use std::process::Command;

fn get_bin_path() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    // Move up from deps/
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("watertight")
}

#[test]
fn test_cli_help_flag() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("Failed to execute binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("watertight"));
    assert!(stdout.contains("check"));
    assert!(stdout.contains("repair"));
    assert!(stdout.contains("remesh"));
    assert!(stdout.contains("decimate"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("install"));
    assert!(stdout.contains("workflows"));
}

#[test]
fn test_cli_doctor_command() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .arg("doctor")
        .output()
        .expect("Failed to execute doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Watertight 3D System & Environment Diagnostics"));
    assert!(stdout.contains("Platform:"));
}

#[test]
fn test_cli_workflows_list_command() {
    let bin = get_bin_path();
    let output = Command::new(&bin)
        .args(["workflows", "list"])
        .output()
        .expect("Failed to execute workflows list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PixelArtistry_01_Image_to_Watertight_Mesh.json"));
    assert!(stdout.contains("PixelArtistry_01b_Image_to_Watertight_Mesh_2K.json"));
    assert!(stdout.contains("PixelArtistry_02_Image_to_GameReady_Asset.json"));
    assert!(stdout.contains("PixelArtistry_03_Your_Mesh_to_GameReady_Asset.json"));
}

#[test]
fn test_cli_workflows_export_command() {
    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir().join(format!("test_cli_wf_export_{}", std::process::id()));

    let output = Command::new(&bin)
        .args(["workflows", "export", temp_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute workflows export");

    assert!(output.status.success());
    assert!(temp_dir.join("PixelArtistry_01_Image_to_Watertight_Mesh.json").exists());
    assert!(temp_dir.join("PixelArtistry_01b_Image_to_Watertight_Mesh_2K.json").exists());
    assert!(temp_dir.join("PixelArtistry_02_Image_to_GameReady_Asset.json").exists());
    assert!(temp_dir.join("PixelArtistry_03_Your_Mesh_to_GameReady_Asset.json").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cli_mesh_pipeline_lifecycle() {
    let bin = get_bin_path();
    let temp_dir = std::env::temp_dir().join(format!("test_cli_lifecycle_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    // 1. Create a non-watertight test OBJ mesh (open cylinder with 2 holes)
    let input_path = temp_dir.join("input_open.obj");
    let repaired_path = temp_dir.join("repaired.obj");
    let remeshed_path = temp_dir.join("remeshed.obj");
    let decimated_path = temp_dir.join("decimated.obj");

    let open_cyl = watertight::Mesh::cylinder(1.0, 2.0, 16, false);
    open_cyl.save(&input_path).unwrap();

    // 2. Test `watertight check`
    let check_out = Command::new(&bin)
        .args(["check", input_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute check");
    assert!(check_out.status.success());
    let check_str = String::from_utf8_lossy(&check_out.stdout);
    assert!(check_str.contains("Watertight Mesh Topological Health Report"));
    assert!(check_str.contains("Boundary Edges (Holes): 32"));

    // 3. Test `watertight repair`
    let repair_out = Command::new(&bin)
        .args([
            "repair",
            input_path.to_str().unwrap(),
            "-o",
            repaired_path.to_str().unwrap(),
            "--res",
            "32",
        ])
        .output()
        .expect("Failed to execute repair");
    assert!(repair_out.status.success());
    assert!(repaired_path.exists());

    // 4. Verify repaired mesh is 100% watertight
    let verify_out = Command::new(&bin)
        .args(["check", repaired_path.to_str().unwrap()])
        .output()
        .expect("Failed to verify repaired mesh");
    assert!(verify_out.status.success());
    let verify_str = String::from_utf8_lossy(&verify_out.stdout);
    assert!(verify_str.contains("100% WATERTIGHT"));

    // 5. Test `watertight remesh`
    let remesh_out = Command::new(&bin)
        .args([
            "remesh",
            input_path.to_str().unwrap(),
            "-o",
            remeshed_path.to_str().unwrap(),
            "--resolution",
            "24",
        ])
        .output()
        .expect("Failed to execute remesh");
    assert!(remesh_out.status.success());
    assert!(remeshed_path.exists());

    // 6. Test `watertight decimate`
    let decimate_out = Command::new(&bin)
        .args([
            "decimate",
            remeshed_path.to_str().unwrap(),
            "-o",
            decimated_path.to_str().unwrap(),
            "--faces",
            "200",
        ])
        .output()
        .expect("Failed to execute decimate");
    assert!(decimate_out.status.success());
    assert!(decimated_path.exists());

    let _ = fs::remove_dir_all(&temp_dir);
}
