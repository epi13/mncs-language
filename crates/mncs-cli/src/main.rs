use std::{env, fs, process::ExitCode};

use mncs_model::Program;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::from(2);
    };

    match command.as_str() {
        "validate" => {
            let Some(path) = args.next() else {
                eprintln!("error: validate requires a manifest path");
                print_usage();
                return ExitCode::from(2);
            };
            if args.next().is_some() {
                eprintln!("error: unexpected additional arguments");
                return ExitCode::from(2);
            }
            validate(&path)
        }
        "--help" | "-h" | "help" => {
            print_usage();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("error: unknown command {other:?}");
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn validate(path: &str) -> ExitCode {
    let input = match fs::read_to_string(path) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("error: unable to read {path:?}: {error}");
            return ExitCode::from(2);
        }
    };

    let program = match Program::from_json(&input) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let report = program.validate();
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("error: unable to serialize validation report: {error}");
            return ExitCode::from(2);
        }
    }

    if report.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn print_usage() {
    eprintln!("MNCS semantic manifest validator");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  mncs validate <manifest.json>");
}
