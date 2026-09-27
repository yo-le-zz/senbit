// installation/lang.rs

use inquire::Select;
use crate::elogln;

fn get_keymap() -> Vec<String> {
    std::fs::read_dir("/usr/share/keymaps/")
        .expect("Répertoire inexistant")
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.to_string())
        })
        .filter(|name| name.ends_with(".bmap"))
        .map(|name| {
            name.trim_end_matches(".bmap").to_string()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

pub fn keyboard_select() -> String {
    let options = get_keymap();

    if options.is_empty() {
        elogln!("Aucun keymap disponible dans /usr/share/keymaps/");
        return "us".to_string();
    }

    Select::new("Select a keyboard layout", options)
        .prompt()
        .unwrap_or_else(|_| "us".to_string())
}