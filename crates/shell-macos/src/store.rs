//! Usage counts on disk. The data is tiny (a few hundred counts), so it is a
//! tab-separated text file replaced atomically, not a database.

use std::io::Write;
use std::path::PathBuf;

use launcher_core::Usage;
use launcher_core::usage::Count;

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/rusty-spotlight/usage.tsv"))
}

pub fn load() -> Usage {
    let mut usage = Usage::default();
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return usage;
    };
    // Lines: `i <id> <c> <last>` or `p <query> <id> <c> <last>`, tab-separated.
    // A malformed line is skipped; the rest of the file still loads.
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["i", id, c, last] => {
                if let (Ok(c), Ok(last)) = (c.parse(), last.parse()) {
                    usage.items.insert(id.to_string(), Count { c, last });
                }
            }
            ["p", q, id, c, last] => {
                if let (Ok(c), Ok(last)) = (c.parse(), last.parse()) {
                    usage.pairs.insert((q.to_string(), id.to_string()), Count { c, last });
                }
            }
            _ => {}
        }
    }
    usage
}

pub fn save(usage: &Usage) -> std::io::Result<()> {
    let Some(path) = path() else { return Ok(()) };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = String::new();
    for (id, k) in &usage.items {
        out.push_str(&format!("i\t{id}\t{}\t{}\n", k.c, k.last));
    }
    for ((q, id), k) in &usage.pairs {
        // A tab or newline in a query would break the format; such a pair is not stored.
        if q.contains(['\t', '\n']) {
            continue;
        }
        out.push_str(&format!("p\t{q}\t{id}\t{}\t{}\n", k.c, k.last));
    }
    let tmp = path.with_extension("tsv.tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(out.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(tmp, path)
}
