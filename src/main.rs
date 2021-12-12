use std::io;
use std::io::prelude::*;

use chaseconv::conversion;

// TODO: add CLI.
fn main() {
    let files: Vec<_> = std::env::args().skip(1).collect();

    if !files.is_empty() {
        eprintln!("Trying to convert {} file(s)...\n", files.len());

        let converters = conversion::converters();

        let items: Vec<_> = converters.iter().map(|converter| converter.name).collect();
        let option = dialoguer::Select::new()
            .with_prompt("Select the format you want to convert the input files to")
            .default(0)
            .items(&items)
            .interact()
            .expect("Failed to select converter option");
        let converter = &converters[option];

        let out_path = dialoguer::Input::new()
            .with_prompt("Select the output directory")
            .default(String::from("output/"))
            .show_default(true)
            .interact()
            .expect("Failed to define output path");

        eprintln!();
        converter.convert(&files, &out_path);
    } else {
        eprintln!("There were no input files. No files were converted.")
    }

    pause();
}

fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line,
    // so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}
