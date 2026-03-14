use std::io;
use std::io::prelude::*;

use chaseconv::conversion;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();

    // Check for CLI flags: --format <glb|gc> --output <dir> [files...]
    let mut format: Option<String> = None;
    let mut output: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    let mut batch = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" | "-f" => {
                i += 1;
                if i < args.len() {
                    format = Some(args[i].clone());
                    batch = true;
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output = Some(args[i].clone());
                }
            }
            _ => {
                files.push(args[i].clone());
            }
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("There were no input files. No files were converted.");
        if !batch {
            pause();
        }
        return;
    }

    eprintln!("Trying to convert {} file(s)...\n", files.len());

    let converters = conversion::converters();

    if batch {
        // CLI mode.
        let format_str = format.unwrap_or_else(|| "glb".to_string());
        let option = match format_str.to_lowercase().as_str() {
            "glb" | "gltf" => 0,
            "gc" | "p3m" | "frm" => 1,
            _ => {
                eprintln!("Unknown format '{}'. Use 'glb' or 'gc'.", format_str);
                return;
            }
        };

        let out_path = output.unwrap_or_else(|| "output/".to_string());
        let converter = &converters[option];
        converter.convert(&files, &out_path);
    } else {
        // Interactive mode.
        let items: Vec<_> = converters.iter().map(|converter| converter.name).collect();
        let option = dialoguer::Select::new()
            .with_prompt("Select the format you want to convert the input files to")
            .default(0)
            .items(&items)
            .interact()
            .expect("Failed to select converter option");
        let converter = &converters[option];

        let out_path: String = match output {
            Some(p) => p,
            None => dialoguer::Input::new()
                .with_prompt("Select the output directory")
                .default(String::from("output/"))
                .show_default(true)
                .interact()
                .expect("Failed to define output path"),
        };

        eprintln!();
        converter.convert(&files, &out_path);

        pause();
    }
}

fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    let _ = stdin.read(&mut [0u8]).unwrap();
}
