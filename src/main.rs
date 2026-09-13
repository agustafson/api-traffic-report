//! Command-line adapter for generating an API traffic report from one log file.

mod report;

use std::{env, fs::File, io::BufReader, path::PathBuf, process};

fn main() {
    let path = match input_path() {
        Ok(path) => path,
        Err(message) => fail(&message),
    };

    let input = match File::open(&path) {
        Ok(input) => input,
        Err(error) => fail(&format!("could not read {}: {error}", path.display())),
    };

    let report = match report::build_report(BufReader::new(input)) {
        Ok(report) => report,
        Err(error) => fail(&format!("could not read {}: {error}", path.display())),
    };
    match serde_json::to_string(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => fail(&format!("could not serialize report: {error}")),
    }
}

fn input_path() -> Result<PathBuf, String> {
    let mut arguments = env::args_os();
    let program = arguments.next().unwrap_or_default();
    let usage = || format!("usage: {} <log-file>", PathBuf::from(&program).display());

    let Some(path) = arguments.next() else {
        return Err(usage());
    };

    if arguments.next().is_some() {
        return Err(usage());
    }

    Ok(PathBuf::from(path))
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    process::exit(1);
}
