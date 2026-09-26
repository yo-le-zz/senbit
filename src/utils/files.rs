use std::path::Path;

pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}