use clap::Parser;
use nerve_waf_crs_import::{count_existing_rules, import_crs, merge_pack_manifest, ImportOptions};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "nerve-waf-crs-import", about = "Import OWASP CRS ModSecurity rules to Nerve JSON packs")]
struct Args {
    /// Path to extracted coreruleset repo (contains rules/REQUEST-*.conf)
    #[arg(long, default_value = "vendor/coreruleset")]
    crs_dir: PathBuf,

    /// Output directory for JSON rule packs
    #[arg(long, default_value = "../rules/crs")]
    out_dir: PathBuf,

    /// Pack version string written into each JSON file
    #[arg(long, default_value = "4.25.0")]
    version: String,

    /// Max paranoia level to import (1=default CRS, 4=paranoid)
    #[arg(long, default_value_t = 2)]
    paranoia_max: u8,

    /// Parse only, do not write files
    #[arg(long)]
    dry_run: bool,

    /// Remove existing JSON packs in out_dir before import
    #[arg(long)]
    clean: bool,
}

fn main() {
    let args = Args::parse();
    if args.clean && args.out_dir.is_dir() && !args.dry_run {
        for entry in glob::glob(&format!("{}/**/*.json", args.out_dir.display())).into_iter().flatten() {
            if let Ok(path) = entry {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    let before = count_existing_rules(&args.out_dir);
    let opts = ImportOptions {
        crs_dir: args.crs_dir,
        out_dir: args.out_dir.clone(),
        version: args.version,
        paranoia_max: args.paranoia_max,
        dry_run: args.dry_run,
        clean: args.clean,
    };

    match import_crs(&opts) {
        Ok(stats) => {
            let after = if args.dry_run {
                stats.rules_imported
            } else {
                count_existing_rules(&args.out_dir)
            };
            eprintln!(
                "CRS import: files={} seen={} imported={} skipped={} compile_failed={} packs={}",
                stats.files_scanned,
                stats.rules_seen,
                stats.rules_imported,
                stats.rules_skipped,
                stats.compile_failed,
                stats.packs_written
            );
            eprintln!("Rules before={before} after={after}");
            if let Ok(manifest) = merge_pack_manifest(&args.out_dir) {
                eprintln!("Packs written: {}", manifest.len());
                for (id, n) in manifest {
                    eprintln!("  {id}: {n} rules");
                }
            }
            if !args.dry_run {
                eprintln!();
                eprintln!("Next: reload WAF rules");
                eprintln!("  curl -X POST http://127.0.0.1:8780/api/rules/packs");
            }
        }
        Err(e) => {
            eprintln!("import failed: {e}");
            std::process::exit(1);
        }
    }
}
