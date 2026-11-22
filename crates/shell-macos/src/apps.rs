//! App discovery (FR-20): find application bundles in the standard locations.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use launcher_core::{Item, Kind};
use objc2_foundation::{NSFileManager, NSString};

fn roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Library/CoreServices/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(Path::new(&home).join("Applications"));
    }
    roots
}

fn walk(dir: &Path, depth: u8, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "app") {
            found.push(path); // never descend into a bundle
        } else if depth > 0 && entry.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&path, depth - 1, found);
        }
    }
}

/// The name Finder shows: localised, without the `.app` extension.
fn display_name(path: &Path) -> String {
    let fm = NSFileManager::defaultManager();
    let name = fm.displayNameAtPath(&NSString::from_str(&path.to_string_lossy())).to_string();
    name.strip_suffix(".app").map(str::to_string).unwrap_or(name)
}

/// `aliases` maps an alias to an app's display name (compared without case).
pub fn scan(aliases: &std::collections::BTreeMap<String, String>) -> Vec<Item> {
    let mut paths = Vec::new();
    for root in roots() {
        walk(&root, 2, &mut paths);
    }
    // Finder lives outside the application folders.
    paths.push(PathBuf::from("/System/Library/CoreServices/Finder.app"));

    let mut seen = HashSet::new();
    let mut items: Vec<Item> = paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .map(|p| {
            let name = display_name(&p);
            let mine: Vec<String> = aliases
                .iter()
                .filter(|(_, app)| app.eq_ignore_ascii_case(&name))
                .map(|(alias, _)| alias.clone())
                .collect();
            Item::new(p.to_string_lossy().into_owned(), name, Kind::App, mine)
        })
        .collect();
    items.sort_by(|a, b| a.title.cmp(&b.title));
    items
}
