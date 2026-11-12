use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use tihulu_ai_core::{scan_directory, DetectedHost, ScanPolicy};

fn usage() {
    eprintln!(
        "TIHULU AI Tools — development CLI (read-only)\n\
         Usage:\n\
           tihulu-ai-cli profile\n\
           tihulu-ai-cli scan <directory> --allow-read\n\
         Scanning never moves, imports, downloads or runs models.\n\
         --allow-read is required for every explicit scan."
    );
}

fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();

    match args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .as_deref()
    {
        Some("profile") if args.next().is_none() => {
            let host = DetectedHost::detect();
            println!("OS: {}", host.os);
            println!("Architecture: {}", host.architecture);
            println!("GPU/NPU validated: {}", host.accelerator_verified);
            ExitCode::SUCCESS
        }
        Some("scan") => {
            let Some(directory) = args.next() else {
                usage();
                return ExitCode::FAILURE;
            };
            let Some(consent) = args.next() else {
                usage();
                return ExitCode::FAILURE;
            };
            if consent != "--allow-read" || args.next().is_some() {
                usage();
                return ExitCode::FAILURE;
            }

            let root = PathBuf::from(directory);
            let policy = ScanPolicy {
                allow_read: true,
                ..ScanPolicy::default()
            };
            match scan_directory(&root, policy) {
                Ok(report) => {
                    println!("Read-only inventory; discovered files are NOT verified models.");
                    println!("Scope: {}", root.display());
                    for candidate in &report.models {
                        println!(
                            "{:?}\t{} bytes\t{}\towner=external, status=unverified",
                            candidate.format,
                            candidate.size_bytes,
                            candidate.path.display()
                        );
                    }
                    println!(
                        "Candidates: {}, skipped symlinks: {}, truncated: {}",
                        report.models.len(),
                        report.skipped_symlinks,
                        report.truncated
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Could not inspect selected directory: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}
