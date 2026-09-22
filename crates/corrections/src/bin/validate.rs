//! `cena-corrections-validate <submission.json> <rooms-dir> <repo-root> [--accept <name>]`
//!
//! Validate a submission against the current map and everything already
//! accepted. Writes a verdict to stdout as Markdown, because its reader is a
//! comment on the issue the file was submitted to.
//!
//! Exit codes are what the workflow branches on: `0` accepted, `1` refused,
//! `2` the tool was called wrongly. A refusal is a **normal outcome**, not a
//! crash -- the submitter gets told why, and nothing is opened.
//!
//! With `--accept <name>`, an accepted submission is also written to
//! `corrections/<name>` and its pictures to `corrections/<stem>/`, ready to
//! be committed to a review branch.

use std::path::Path;
use std::process::ExitCode;

use cena_corrections::disk;
use cena_corrections::file::{parse, pictures};
use cena_corrections::fold::fold;
use cena_corrections::validate::{summary, validate};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (positional, accept) = split(&args);
    let [submission, rooms, root] = positional.as_slice() else {
        eprintln!(
            "usage: cena-corrections-validate <submission.json> <rooms-dir> <repo-root> \
             [--accept <name>]"
        );
        return ExitCode::from(2);
    };

    match run(
        Path::new(submission),
        Path::new(rooms),
        Path::new(root),
        accept.as_deref(),
    ) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            // A tool failure, not a refusal: say so in the same Markdown the
            // workflow posts, so it never looks like the submitter's fault.
            println!("## Could not check this submission\n\n```\n{error}\n```");
            ExitCode::from(2)
        }
    }
}

fn split(args: &[String]) -> (Vec<String>, Option<String>) {
    let mut positional = Vec::new();
    let mut accept = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--accept" {
            accept = rest.next().cloned();
        } else {
            positional.push(arg.clone());
        }
    }
    (positional, accept)
}

fn run(
    submission: &Path,
    rooms: &Path,
    root: &Path,
    accept: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(submission)?;

    // A parse failure is its own outcome: the attachment is not a corrections
    // file at all, which needs different words than a correction that cannot
    // be applied.
    let correction = match parse(&text) {
        Ok(correction) => correction,
        Err(error) => {
            println!("## This attachment could not be read\n\n{error}\n");
            println!(
                "\nIt should be the `.json` file `hydra-mapper` exports, attached as-is. If you \
                 renamed it or pasted its contents into the issue body, attach the original file \
                 instead."
            );
            return Ok(false);
        }
    };

    let map = disk::current_map(rooms)?;
    let already = fold(&disk::accepted(root)?);
    let report = validate(&correction, &map, &already.folded);

    if !report.refusals.is_empty() {
        println!("## This submission was not accepted\n");
        for refusal in &report.refusals {
            println!("- {refusal}");
        }
        println!(
            "\nNothing has been merged. Correct the export and attach it to this issue again -- \
             the check runs on every new attachment."
        );
        return Ok(false);
    }

    println!("## Validated\n");
    println!("From `{}`.\n", correction.generator);
    for (what, count) in summary(&correction) {
        println!("- {count} {what}");
    }
    if !report.warnings.is_empty() {
        println!("\n### Worth a reviewer's eye\n");
        for warning in &report.warnings {
            println!("- {warning}");
        }
    }

    if let Some(name) = accept {
        let path = disk::write(root, name, &correction)?;
        println!("\nWritten to `{}`.", rel(root, &path));

        let svgs = disk::write_pictures(
            &root
                .join(disk::DIR)
                .join(Path::new(name).with_extension("")),
            &pictures(&text)?,
        )?;
        if !svgs.is_empty() {
            println!(
                "\n{} picture(s) are on the review branch so they can be viewed in the pull \
                 request. They are evidence, not corrections, and the merge does not keep them.",
                svgs.len()
            );
        }
    }

    Ok(true)
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
