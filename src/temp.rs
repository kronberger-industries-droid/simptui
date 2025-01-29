use std::fs;
use std::io;

fn get_files(dir_path: &str) -> io::Result<Vec<String>> {
    let entries = fs::read_dir(dir_path)?;

    let mut filenames: Vec<String> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
                filenames.push(filename.to_string());
            }
        }
    }
    Ok(filenames)
}

fn main() -> io::Result<()> {
    let dir_path = "./";

    let filenames = get_files(dir_path)?;

    for filename in filenames {
        println!("{}", filename);
    }

    Ok(())
}
