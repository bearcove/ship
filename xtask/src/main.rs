use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("codegen") => codegen_typescript(&workspace_root()?),
        Some("install") => {
            install(&workspace_root()?);
            Ok(())
        }
        Some("help") | Some("--help") | Some("-h") => {
            print_usage();
            Ok(())
        }
        Some(other) => Err(format!("unknown xtask command: {other}").into()),
        None => {
            print_usage();
            Ok(())
        }
    }
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest_dir
        .parent()
        .ok_or("xtask manifest should be inside workspace root")?;
    Ok(root.to_path_buf())
}

fn print_usage() {
    eprintln!("Usage: cargo xtask <command>");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  codegen    Generate TypeScript bindings from the Ship roam service trait");
    eprintln!("  install    Build and install ship binaries to ~/.cargo/bin");
}

fn build_frontend(workspace_root: &Path) {
    let frontend_dir = workspace_root.join("frontend");

    println!("Installing frontend dependencies...");
    let status = Command::new("pnpm")
        .arg("install")
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to run pnpm install");
    if !status.success() {
        eprintln!("Error: pnpm install failed");
        std::process::exit(status.code().unwrap_or(1));
    }

    println!("Building frontend...");
    let status = Command::new("pnpm")
        .arg("build-no-check")
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to run pnpm build");
    if !status.success() {
        eprintln!("Error: pnpm build failed");
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn install(workspace_root: &Path) {
    let binaries = ["ship"];

    build_frontend(workspace_root);

    println!("Building ship binaries...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(workspace_root)
        .status()
        .expect("Failed to run cargo build");

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    let release_dir = workspace_root.join("target/release");

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .expect("Neither HOME nor USERPROFILE is set");
    let install_dir = PathBuf::from(home).join(".cargo").join("bin");

    for binary_name in binaries {
        let src = if cfg!(windows) {
            release_dir.join(format!("{binary_name}.exe"))
        } else {
            release_dir.join(binary_name)
        };
        let dst = if cfg!(windows) {
            install_dir.join(format!("{binary_name}.exe"))
        } else {
            install_dir.join(binary_name)
        };

        if !src.exists() {
            eprintln!(
                "Warning: {} not found in target/release, skipping",
                binary_name
            );
            continue;
        }

        fs::copy(&src, &dst).unwrap_or_else(|_| panic!("Failed to copy {}", binary_name));
        println!("Copied {} to {}", binary_name, dst.display());

        // On macOS, codesign the installed binary to avoid AMFI issues.
        // Signing must happen AFTER copy, not before.
        #[cfg(target_os = "macos")]
        {
            let dst_str = dst.to_str().expect("non-UTF-8 path");
            let status = Command::new("codesign")
                .args(["--sign", "-", "--force", dst_str])
                .status()
                .expect("Failed to run codesign");

            if !status.success() {
                eprintln!(
                    "Warning: codesign failed for {}, continuing anyway",
                    binary_name
                );
            }
        }

        // Verify the installed binary works.
        let output = Command::new(&dst)
            .arg("--version")
            .output()
            .unwrap_or_else(|_| panic!("Failed to run {} --version", binary_name));

        if !output.status.success() {
            eprintln!("Error: {} --version failed", binary_name);
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            std::process::exit(1);
        }

        let version = String::from_utf8_lossy(&output.stdout);
        println!("Installed: {}", version.trim());
    }
}

// r[frontend.codegen]
// r[dep.roam-codegen]
// r[backend.rpc]
fn codegen_typescript(workspace_root: &Path) -> Result<(), Box<dyn Error>> {
    let out_dir = workspace_root
        .join("frontend")
        .join("src")
        .join("generated");
    std::fs::create_dir_all(&out_dir)?;

    let descriptor = ship_service::ship_service_descriptor();
    let code = normalize_generated_typescript(roam_codegen::targets::typescript::generate_service(
        descriptor,
    ));

    let out_path = out_dir.join("ship.ts");
    write_if_changed(&out_path, code)?;

    let index_path = out_dir.join("index.ts");
    write_if_changed(&index_path, "export * from \"./ship\";\n".to_owned())?;

    println!("generated {}", out_path.display());
    println!("generated {}", index_path.display());
    Ok(())
}

fn normalize_generated_typescript(code: String) -> String {
    let mut code = code;

    if code.contains("import { Tx, Rx, bindChannels } from \"@bearcove/roam-core\";")
        && !code.contains("Rx<")
    {
        code = code.replace(
            "import { Tx, Rx, bindChannels } from \"@bearcove/roam-core\";",
            "import { Tx, bindChannels } from \"@bearcove/roam-core\";",
        );
    }

    if code.contains("initial_credit: 16, ") {
        code = code.replace("initial_credit: 16, ", "");
    }

    code
}

fn write_if_changed(path: &Path, content: String) -> Result<(), Box<dyn Error>> {
    match std::fs::read_to_string(path) {
        Ok(existing) if existing == content => Ok(()),
        Ok(_) | Err(_) => {
            std::fs::write(path, content)?;
            Ok(())
        }
    }
}
