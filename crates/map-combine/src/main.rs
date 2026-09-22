//! `cena-map-combine <conversion-dir> <out.map>`

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use cena_map_combine::combine::{combine, write};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, out] = args.as_slice() else {
        eprintln!("usage: cena-map-combine <conversion-dir> <out.map>");
        return ExitCode::from(2);
    };
    match run(Path::new(dir), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cena-map-combine: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(dir: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let map = combine(dir)?;
    let read = started.elapsed();

    let written = write(&map, out)?;

    // How long the client will wait for this file, measured on it rather than
    // guessed: read the bytes, decode them, build the indexes.
    let started = Instant::now();
    let loaded = cena_map::binary::decode(&std::fs::read(out)?)?;
    let load = started.elapsed();

    println!("rooms      {}", written.rooms);
    println!("file       {} bytes  ({})", written.bytes, out.display());
    println!("read json  {read:.2?}");
    println!("load map   {load:.2?}  ({} rooms)", loaded.len());
    Ok(())
}
