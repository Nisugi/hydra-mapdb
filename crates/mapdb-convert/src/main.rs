//! `cena-mapdb-convert <upstream-map.json> <out-dir>`

use std::path::Path;
use std::process::ExitCode;

use cena_mapdb_convert::{output, run};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [upstream, out] = args.as_slice() else {
        eprintln!("usage: cena-mapdb-convert <upstream-map.json> <out-dir>");
        return ExitCode::from(2);
    };
    match convert(Path::new(upstream), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cena-mapdb-convert: {error}");
            ExitCode::FAILURE
        }
    }
}

fn convert(upstream: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(upstream)?;
    let conversion = run::convert(&text)?;
    let written = output::write(out, &conversion)?;
    print!("{}", conversion.report.summary());
    println!(
        "files                 {} created, {} updated, {} unchanged",
        written.created, written.updated, written.unchanged
    );
    Ok(())
}
