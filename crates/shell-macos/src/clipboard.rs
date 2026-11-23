//! Clipboard history. Text, images and copied files are kept on disk and
//! searched from the panel. Images are resized before they are stored, and
//! only a small thumbnail is decoded to draw a row, so a photo does not sit
//! in memory.
//!
//! Password managers mark a copy as concealed. Those copies are skipped.
//! Putting an item back on the clipboard does not record it a second time.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use launcher_core::text::fold;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{NSArray, NSData, NSString};
use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 250;
const MAX_IMAGE_BYTES: u64 = 150 * 1024 * 1024;
const MAX_TEXT: usize = 512 * 1024;
const MAX_IMAGE_READ: usize = 40 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKind {
    Text,
    Image,
    Files,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub kind: ClipKind,
    pub created: f64,
    pub pinned: bool,
    pub preview: String,
    pub detail: String,
    pub hash: u64,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    /// File name inside `items/`, for text and images.
    blob: Option<String>,
    pub files: Vec<String>,
}

pub struct History {
    dir: PathBuf,
    entries: Vec<Entry>,
}

fn root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/rusty-spotlight/clipboard"))
}

impl History {
    fn load() -> Self {
        match root() {
            Some(dir) => Self::open(dir),
            None => Self { dir: PathBuf::new(), entries: Vec::new() },
        }
    }

    fn open(dir: PathBuf) -> Self {
        let path = dir.join("index.json");
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<Entry>>(&b).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.blob.as_ref().is_none_or(|name| dir.join("items").join(name).is_file()) || e.kind == ClipKind::Files)
            .collect();
        let mut history = Self { dir, entries };
        history.sort();
        history
    }

    fn search(&self, filter: &str) -> Vec<Entry> {
        let needle = fold(filter.trim());
        self.entries
            .iter()
            .filter(|e| {
                needle.is_empty()
                    || fold(&e.preview).contains(&needle)
                    || fold(&e.detail).contains(&needle)
                    || e.files.iter().any(|p| fold(p).contains(&needle))
            })
            .cloned()
            .collect()
    }

    fn insert(&mut self, entry: Entry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.hash == entry.hash && e.kind == entry.kind) {
            existing.created = entry.created;
            if entry.blob.is_some() && entry.blob != existing.blob {
                delete_blob(&self.dir, &entry);
            }
        } else {
            self.entries.push(entry);
            self.evict();
        }
        self.sort();
        let _ = self.save();
    }

    fn sort(&mut self) {
        self.entries.sort_by(|a, b| {
            b.pinned.cmp(&a.pinned).then_with(|| b.created.partial_cmp(&a.created).unwrap_or(std::cmp::Ordering::Equal))
        });
    }

    fn evict(&mut self) {
        while self.entries.len() > MAX_ENTRIES {
            let Some(i) = self.oldest_unpinned(|_| true) else { break };
            self.remove_at(i);
        }
        while self.image_bytes() > MAX_IMAGE_BYTES {
            let Some(i) = self.oldest_unpinned(|e| e.kind == ClipKind::Image) else { break };
            self.remove_at(i);
        }
    }

    fn oldest_unpinned(&self, pred: impl Fn(&Entry) -> bool) -> Option<usize> {
        self.entries.iter().enumerate().rev().find(|(_, e)| !e.pinned && pred(e)).map(|(i, _)| i)
    }

    fn image_bytes(&self) -> u64 {
        self.entries.iter().filter(|e| e.kind == ClipKind::Image).map(|e| e.bytes).sum()
    }

    fn remove_at(&mut self, index: usize) {
        let entry = self.entries.remove(index);
        delete_blob(&self.dir, &entry);
    }

    fn pin(&mut self, id: &str) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) else { return false };
        entry.pinned = !entry.pinned;
        self.sort();
        let _ = self.save();
        true
    }

    fn delete(&mut self, id: &str) -> bool {
        let Some(index) = self.entries.iter().position(|e| e.id == id) else { return false };
        self.remove_at(index);
        let _ = self.save();
        true
    }

    fn clear_unpinned(&mut self) {
        let doomed: Vec<usize> = self.entries.iter().enumerate().rev().filter(|(_, e)| !e.pinned).map(|(i, _)| i).collect();
        for i in doomed {
            self.remove_at(i);
        }
        let _ = self.save();
    }

    fn save(&self) -> std::io::Result<()> {
        if self.dir.as_os_str().is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(self.dir.join("items"))?;
        let path = self.dir.join("index.json");
        let tmp = self.dir.join("index.json.tmp");
        let data = serde_json::to_vec_pretty(&self.entries).unwrap_or_else(|_| b"[]".to_vec());
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&data)?;
        file.sync_all()?;
        std::fs::rename(tmp, path)
    }

    fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

fn delete_blob(dir: &Path, entry: &Entry) {
    let Some(name) = &entry.blob else { return };
    let items = dir.join("items");
    let _ = std::fs::remove_file(items.join(name));
    if let Some(stem) = name.split('.').next() {
        let _ = std::fs::remove_file(items.join(format!("{stem}.thumb.png")));
    }
}

fn hash_bytes(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn new_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{:.0}-{n}", now() * 1000.0)
}

thread_local! {
    static HISTORY: std::cell::RefCell<History> = std::cell::RefCell::new(History::load());
    static LAST: Cell<isize> = const { Cell::new(0) };
    static SUPPRESS: Cell<isize> = const { Cell::new(0) };
}

pub fn search(filter: &str) -> Vec<Entry> {
    HISTORY.with(|h| h.borrow().search(filter))
}

/// `clip` or `clipboard`, optionally followed by a filter. `None` when the
/// query is an ordinary search.
pub fn keyword_filter(query: &str) -> Option<String> {
    let trimmed = query.trim();
    let (word, rest) = trimmed.split_once(char::is_whitespace).unwrap_or((trimmed, ""));
    if matches!(fold(word).as_str(), "clip" | "clipboard") {
        Some(rest.trim().to_string())
    } else {
        None
    }
}

pub fn pin(id: &str) -> bool {
    HISTORY.with(|h| h.borrow_mut().pin(id))
}

pub fn delete(id: &str) -> bool {
    HISTORY.with(|h| h.borrow_mut().delete(id))
}

pub fn clear_unpinned() {
    HISTORY.with(|h| h.borrow_mut().clear_unpinned());
}

pub fn thumb_path(id: &str) -> Option<PathBuf> {
    HISTORY.with(|h| {
        let history = h.borrow();
        let entry = history.get(id)?;
        let name = entry.blob.as_ref()?.trim_end_matches(".png");
        let path = history.dir.join("items").join(format!("{name}.thumb.png"));
        path.is_file().then_some(path)
    })
}

/// Read the general pasteboard when its change count has moved. Returns
/// whether a new item was stored.
pub fn poll() -> bool {
    let pasteboard = NSPasteboard::generalPasteboard();
    let count = pasteboard.changeCount();
    if count == LAST.get() {
        return false;
    }
    if count == SUPPRESS.get() {
        LAST.set(count);
        return false;
    }
    LAST.set(count);
    let Some(entry) = capture(&pasteboard) else { return false };
    HISTORY.with(|h| h.borrow_mut().insert(entry));
    true
}

/// Put `id` back on the clipboard. The following pasteboard change is ignored.
pub fn copy_entry(id: &str) -> Result<(), String> {
    let entry = HISTORY.with(|h| h.borrow().get(id).cloned()).ok_or("that clipboard item is gone")?;
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    let ok = match entry.kind {
        ClipKind::Text => {
            let path = blob_path(&entry).ok_or("the copied text is missing")?;
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            pasteboard.setString_forType(&NSString::from_str(&text), unsafe { NSPasteboardTypeString })
        }
        ClipKind::Image => {
            let path = blob_path(&entry).ok_or("the copied image is missing")?;
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let data = NSData::with_bytes(&bytes);
            pasteboard.setData_forType(Some(&data), &NSString::from_str("public.png"))
        }
        ClipKind::Files => {
            if entry.files.is_empty() {
                return Err("that copy has no files".into());
            }
            let names: Vec<Retained<NSString>> = entry.files.iter().map(|p| NSString::from_str(p)).collect();
            let refs: Vec<&NSString> = names.iter().map(|n| n.as_ref()).collect();
            let list = NSArray::from_slice(&refs);
            unsafe {
                let plist: &AnyObject = &*Retained::as_ptr(&list).cast();
                pasteboard.setPropertyList_forType(plist, &NSString::from_str("NSFilenamesPboardType"))
            }
        }
    };
    if !ok {
        return Err("the clipboard did not accept it".into());
    }
    SUPPRESS.set(pasteboard.changeCount());
    LAST.set(SUPPRESS.get());
    Ok(())
}

fn blob_path(entry: &Entry) -> Option<PathBuf> {
    let name = entry.blob.as_ref()?;
    HISTORY.with(|h| {
        let path = h.borrow().dir.join("items").join(name);
        path.is_file().then_some(path)
    })
}

fn capture(pasteboard: &NSPasteboard) -> Option<Entry> {
    let types = pasteboard.types()?;
    let names: Vec<String> = types.iter().map(|t| t.to_string()).collect();
    if names.iter().any(|t| {
        t.contains("ConcealedType") || t.contains("TransientType") || t.contains("AutoGeneratedType")
    }) {
        return None;
    }
    if let Some(files) = file_list(pasteboard, &names) {
        if !files.is_empty() {
            return Some(files_entry(files));
        }
    }
    for kind in ["public.png", "public.jpeg", "public.tiff", "image/png", "image/tiff"] {
        if names.iter().any(|t| t == kind) {
            if let Some(data) = pasteboard.dataForType(&NSString::from_str(kind)) {
                if data.length() > MAX_IMAGE_READ {
                    return None;
                }
                let bytes = data_bytes(&data);
                if let Some(entry) = store_image(&bytes) {
                    return Some(entry);
                }
            }
        }
    }
    let text = pasteboard.stringForType(unsafe { NSPasteboardTypeString })?.to_string();
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(store_text(text))
}

fn file_list(pasteboard: &NSPasteboard, types: &[String]) -> Option<Vec<String>> {
    if !types.iter().any(|t| t == "NSFilenamesPboardType" || t == "public.file-url") {
        return None;
    }
    if let Some(list) = pasteboard.propertyListForType(&NSString::from_str("NSFilenamesPboardType")) {
        if let Ok(list) = list.downcast::<NSArray>() {
            let mut files = Vec::new();
            for index in 0..list.count() {
                let item = list.objectAtIndex(index);
                let Some(name) = item.downcast_ref::<NSString>() else { continue };
                let path = name.to_string();
                if !path.is_empty() {
                    files.push(path);
                }
            }
            if !files.is_empty() {
                return Some(files);
            }
        }
    }
    let url = pasteboard.stringForType(&NSString::from_str("public.file-url"))?.to_string();
    let path = url.strip_prefix("file://").unwrap_or(&url);
    let path = urlencoding_decode(path);
    Some(vec![path])
}

fn urlencoding_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn files_entry(files: Vec<String>) -> Entry {
    let preview = if files.len() == 1 {
        Path::new(&files[0]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| files[0].clone())
    } else {
        format!("{} files", files.len())
    };
    let detail = if files.len() == 1 {
        pretty_parent(&files[0])
    } else {
        format!("{preview}")
    };
    let hash = hash_bytes(files.join("\n").as_bytes());
    Entry {
        id: new_id(),
        kind: ClipKind::Files,
        created: now(),
        pinned: false,
        preview,
        detail,
        hash,
        bytes: 0,
        width: 0,
        height: 0,
        blob: None,
        files,
    }
}

fn store_text(text: &str) -> Entry {
    let stored: String = text.chars().take(MAX_TEXT).collect();
    let id = new_id();
    let name = format!("{id}.txt");
    let preview = stored.lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(160).collect::<String>();
    let preview = if preview.is_empty() { stored.chars().take(160).collect() } else { preview };
    let lines = stored.lines().count().max(1);
    let chars = stored.chars().count();
    let detail = if lines > 1 {
        format!("{lines} lines · {chars} characters")
    } else {
        format!("{chars} characters")
    };
    if let Some(dir) = root() {
        let items = dir.join("items");
        let _ = std::fs::create_dir_all(&items);
        let _ = std::fs::write(items.join(&name), &stored);
    }
    Entry {
        id,
        kind: ClipKind::Text,
        created: now(),
        pinned: false,
        preview,
        detail,
        hash: hash_bytes(stored.as_bytes()),
        bytes: stored.len() as u64,
        width: 0,
        height: 0,
        blob: Some(name),
        files: Vec::new(),
    }
}

fn store_image(bytes: &[u8]) -> Option<Entry> {
    let dir = root()?;
    let items = dir.join("items");
    std::fs::create_dir_all(&items).ok()?;
    let id = new_id();
    let src = items.join(format!("{id}.src"));
    std::fs::write(&src, bytes).ok()?;
    let png = items.join(format!("{id}.png"));
    let thumb = items.join(format!("{id}.thumb.png"));
    let shrunk = Command::new("/usr/bin/sips")
        .args(["-s", "format", "png", "-Z", "1600", &src.to_string_lossy(), "--out", &png.to_string_lossy()])
        .output()
        .ok()
        .is_some_and(|o| o.status.success() && png.is_file());
    let _ = std::fs::remove_file(&src);
    if !shrunk {
        let _ = std::fs::remove_file(&png);
        return None;
    }
    let _ = Command::new("/usr/bin/sips")
        .args(["-s", "format", "png", "-Z", "144", &png.to_string_lossy(), "--out", &thumb.to_string_lossy()])
        .status();
    let (width, height) = image_size(&png);
    let bytes = std::fs::metadata(&png).map(|m| m.len()).unwrap_or(0);
    let blob = format!("{id}.png");
    Some(Entry {
        id,
        kind: ClipKind::Image,
        created: now(),
        pinned: false,
        preview: "Image".into(),
        detail: if width > 0 { format!("{width}×{height}") } else { "Image".into() },
        hash: hash_bytes(&std::fs::read(&png).unwrap_or_default()),
        bytes,
        width,
        height,
        blob: Some(blob),
        files: Vec::new(),
    })
}

fn image_size(path: &Path) -> (u32, u32) {
    let output = Command::new("/usr/bin/sips").args(["-g", "pixelWidth", "-g", "pixelHeight", &path.to_string_lossy()]).output();
    let Ok(output) = output else { return (0, 0) };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut width = 0;
    let mut height = 0;
    for line in text.lines() {
        if let Some(v) = line.split(':').nth(1) {
            let n: u32 = v.trim().parse().unwrap_or(0);
            if line.contains("pixelWidth") {
                width = n;
            } else if line.contains("pixelHeight") {
                height = n;
            }
        }
    }
    (width, height)
}

fn data_bytes(data: &NSData) -> Vec<u8> {
    data.to_vec()
}

pub fn pretty_parent(path: &str) -> String {
    let parent = Path::new(path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    shorten(&parent)
}

pub fn shorten(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: ClipKind, preview: &str, created: f64, pinned: bool) -> Entry {
        Entry {
            id: preview.into(),
            kind,
            created,
            pinned,
            preview: preview.into(),
            detail: String::new(),
            hash: hash_bytes(preview.as_bytes()),
            bytes: 0,
            width: 0,
            height: 0,
            blob: None,
            files: Vec::new(),
        }
    }

    #[test]
    fn clip_is_a_keyword_and_clipping_is_not() {
        assert_eq!(keyword_filter("clip").as_deref(), Some(""));
        assert_eq!(keyword_filter("  Clipboard  notes ").as_deref(), Some("notes"));
        assert_eq!(keyword_filter("clipping"), None);
        assert_eq!(keyword_filter("eclipse"), None);
    }

    #[test]
    fn search_is_case_insensitive_and_pins_stay_first() {
        let dir = std::env::temp_dir().join(format!("rs-clip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut history = History::open(dir);
        history.insert(entry(ClipKind::Text, "Quarterly report", 1.0, false));
        history.insert(entry(ClipKind::Text, "hello", 2.0, false));
        history.insert(entry(ClipKind::Image, "Image", 3.0, true));
        let found = history.search("REPORT");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].preview, "Quarterly report");
        let all = history.search("");
        assert!(all[0].pinned);
        assert_eq!(all[1].preview, "hello");
    }

    #[test]
    fn a_repeat_copy_updates_the_existing_item() {
        let dir = std::env::temp_dir().join(format!("rs-clip-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut history = History::open(dir);
        history.insert(entry(ClipKind::Text, "same", 1.0, false));
        history.insert(entry(ClipKind::Text, "same", 5.0, false));
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].created, 5.0);
    }

    #[test]
    fn eviction_drops_the_oldest_unpinned_item() {
        let dir = std::env::temp_dir().join(format!("rs-clip-evict-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut history = History::open(dir);
        history.insert(entry(ClipKind::Text, "keep", 1.0, true));
        for n in 0..MAX_ENTRIES {
            history.insert(entry(ClipKind::Text, &format!("n{n}"), n as f64 + 2.0, false));
        }
        assert!(history.entries.len() <= MAX_ENTRIES);
        assert!(history.entries.iter().any(|e| e.pinned && e.preview == "keep"));
    }
}
