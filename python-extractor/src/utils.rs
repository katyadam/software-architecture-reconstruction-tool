use std::fs;

#[allow(dead_code)]
pub fn load_file(filename: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(filename)
}
