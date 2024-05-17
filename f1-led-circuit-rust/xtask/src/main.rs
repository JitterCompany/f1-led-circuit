use clap::{arg, Command};
use std::process::Command as ProcessCommand;

fn main() {
    let matches = Command::new("Xtask")
        .version("0.1.0")
        .about("Custom build tasks for the project")
        .arg(arg!(--bin <NAME> "Run a binary from f1-hardware or f1-logic").required(true))
        .get_matches();

    if let Some(bin) = matches.get_one::<String>("bin") {
        if let Err(e) = run_bin(bin) {
            eprintln!("Failed to run binary: {:?}", e);
        }
    } else {
        eprintln!("Please specify a valid --bin name.");
    }
}

fn run_bin(bin: &str) -> std::io::Result<()> {
    // Determine the crate to run the binary from
    let crate_dir = if bin == "embassy_hello_world" {
        "f1-hardware"
    } else {
        "f1-logic"
    };

    let current_dir = std::env::current_dir()?;
    let crate_path = current_dir.join(crate_dir);
    println!("Running binary '{}' in directory '{}'", bin, crate_path.display());

    let status = ProcessCommand::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg(bin)
        .arg("--target")
        .arg("riscv32imc-unknown-none-elf")
        .arg("--features")
        .arg("esp32c3")
        .current_dir(&crate_path)
        .status()?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
