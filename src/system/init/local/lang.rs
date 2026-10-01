use std::fs;
use std::io;
use std::path::Path;

pub fn get_lang(root: &str) -> io::Result<String> {
    let path = Path::new(root).join("etc/default/locale");

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok("C".to_string());
        }
        Err(e) => return Err(e),
    };

    for line in content.lines() {
        let line = line.trim();

        if let Some(value) = line.strip_prefix("LANG=") {
            return Ok(value.trim().trim_matches('"').to_string());
        }
    }

    Ok("C".to_string())
}