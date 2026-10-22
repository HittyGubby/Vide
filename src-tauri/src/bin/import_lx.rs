use std::path::Path;

use serde_json::Value;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let json_path = match args.next() {
        Some(value) => value,
        None => {
            eprintln!("usage: import_lx <lx_list.json> [vide.db]");
            std::process::exit(2);
        }
    };
    let db_path = args.next().unwrap_or_else(|| {
        dirs::data_dir()
            .map(|dir| dir.join("vide").join("vide.db"))
            .unwrap_or_else(|| "vide.db".into())
            .to_string_lossy()
            .to_string()
    });

    let content = match std::fs::read_to_string(&json_path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("read {json_path}: {error}");
            std::process::exit(1);
        }
    };
    let data: Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("parse {json_path}: {error}");
            std::process::exit(1);
        }
    };
    let report = match vide_lib::lx_import::import_lx_list_at(Path::new(&db_path), &data).await {
        Ok(report) => report,
        Err(error) => {
            eprintln!("import failed: {error}");
            std::process::exit(1);
        }
    };
    match serde_json::to_string_pretty(&report) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("serialize report: {error}"),
    }
}
