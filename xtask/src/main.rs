use std::error::Error;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("codegen") => codegen_typescript(&workspace_root()?),
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
    if code.contains("import { Tx, Rx, bindChannels } from \"@bearcove/roam-core\";")
        && !code.contains("Rx<")
    {
        return code.replace(
            "import { Tx, Rx, bindChannels } from \"@bearcove/roam-core\";",
            "import { Tx, bindChannels } from \"@bearcove/roam-core\";",
        );
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
