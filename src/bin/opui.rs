use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use opui::{CheckOptions, canonical_bytes, check_path, format_diagnostics, parse_strict};

#[derive(Parser)]
#[command(name = "opui", about = "openpencil-runtime-ui checker")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        files: Vec<PathBuf>,
    },
    Canonicalize {
        input: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { files } => {
            if files.is_empty() {
                eprintln!("opui check: FILE required");
                return ExitCode::from(2);
            }
            let mut failed = false;
            let multi = files.len() > 1;
            for file in &files {
                let diags = check_path(file, &CheckOptions::for_path(file));
                if diags.iter().any(|d| d.severity == "error") {
                    failed = true;
                }
                let text = format_diagnostics(&diags);
                if text.is_empty() {
                    continue;
                }
                if multi {
                    println!("{file}:", file = file.display());
                }
                print!("{text}");
            }
            if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Command::Canonicalize { input, output } => match std::fs::read(&input) {
            Ok(bytes) => match parse_strict(&bytes) {
                Ok((value, _)) => {
                    let out = canonical_bytes(&value);
                    match output {
                        Some(path) => {
                            if let Err(err) = std::fs::write(&path, out) {
                                eprintln!("opui canonicalize: {err}");
                                return ExitCode::from(1);
                            }
                        }
                        None => {
                            if io::stdout().write_all(&out).is_err() {
                                return ExitCode::from(1);
                            }
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(diag) => {
                    eprint!("{}", format_diagnostics(&[diag]));
                    ExitCode::from(1)
                }
            },
            Err(err) => {
                eprintln!("opui canonicalize: {err}");
                ExitCode::from(1)
            }
        },
    }
}
