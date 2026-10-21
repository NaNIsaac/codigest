//! Git-blob-hash and stat based cache of file blocks and digest summaries, in the same
//! `.codigest/cache.json` the Python build reads and writes (`core/cache.py`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{Map, Value, json};
use sha1::{Digest, Sha1};

/// Bump when the shape of a cached block or summary changes, so stale ones are dropped.
pub const FORMAT: u64 = 2;

/// The SHA-1 git gives a blob with these bytes.
pub fn compute_git_blob_hash(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", data.len()));
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

pub fn get_file_blob_hash(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|data| compute_git_blob_hash(&data))
}

/// `str(bool)`, the key under which a block of each numbering is stored.
fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

struct Stat {
    mtime_ns: i64,
    size: u64,
}

fn stat(path: &Path) -> Option<Stat> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Stat {
        mtime_ns: i64::try_from(mtime.as_nanos()).ok()?,
        size: meta.len(),
    })
}

fn matches(entry: &Value, stat: &Stat) -> bool {
    entry.get("mtime_ns").and_then(Value::as_i64) == Some(stat.mtime_ns)
        && entry.get("size").and_then(Value::as_u64) == Some(stat.size)
}

pub struct ContentCache {
    cache_file: PathBuf,
    data: Map<String, Value>,
    pub hits: u32,
    pub misses: u32,
}

impl ContentCache {
    pub fn new(root: &Path) -> Self {
        let cache_file = root.join(".codigest").join("cache.json");
        let mut data = fs::read_to_string(&cache_file)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| match value {
                Value::Object(map) => Some(map),
                _ => None,
            })
            .unwrap_or_default();
        if data.get("format").and_then(Value::as_u64) != Some(FORMAT) {
            data = Map::new();
            data.insert("format".to_owned(), json!(FORMAT));
        }
        Self {
            cache_file,
            data,
            hits: 0,
            misses: 0,
        }
    }

    /// Only in projects that already have `.codigest/` (init or mark): reading must not
    /// leave a folder behind in a project that never asked for one.
    /// Whether `save` will write anything: only in a project that already has `.codigest/`.
    pub fn persists(&self) -> bool {
        self.cache_file.parent().is_some_and(Path::is_dir)
    }

    pub fn save(&self) {
        if !self.cache_file.parent().is_some_and(Path::is_dir) {
            return;
        }
        if let Ok(text) = serde_json::to_string_pretty(&self.data) {
            let _ = fs::write(&self.cache_file, text);
        }
    }

    fn files(&self, section: &str, rel_path: &str) -> Option<&Value> {
        self.data.get(section)?.get(rel_path)
    }

    pub fn get_file_block(
        &mut self,
        rel_path: &str,
        blob_hash: Option<&str>,
        line_numbers: bool,
        file_path: Option<&Path>,
    ) -> Option<String> {
        let key = py_bool(line_numbers);
        let block_of = |entry: &Value| {
            entry
                .get("blocks")
                .and_then(|b| b.get(key))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };

        // Fast path: the file's size and mtime have not changed.
        if let Some(now) = file_path.and_then(stat)
            && let Some(block) = self
                .files("files", rel_path)
                .filter(|entry| matches(entry, &now))
                .and_then(block_of)
        {
            self.hits += 1;
            return Some(block);
        }

        if let Some(hash) = blob_hash.filter(|h| !h.is_empty()) {
            let found = self
                .files("files", rel_path)
                .filter(|entry| entry.get("blob_hash").and_then(Value::as_str) == Some(hash))
                .and_then(block_of);
            if let Some(block) = found {
                self.hits += 1;
                if let Some(now) = file_path.and_then(stat)
                    && let Some(entry) = self.file_entry_mut(rel_path)
                {
                    entry.insert("mtime_ns".to_owned(), json!(now.mtime_ns));
                    entry.insert("size".to_owned(), json!(now.size));
                }
                return Some(block);
            }
        }

        self.misses += 1;
        None
    }

    fn file_entry_mut(&mut self, rel_path: &str) -> Option<&mut Map<String, Value>> {
        self.data
            .get_mut("files")?
            .get_mut(rel_path)?
            .as_object_mut()
    }

    pub fn set_file_block(
        &mut self,
        rel_path: &str,
        blob_hash: Option<&str>,
        line_numbers: bool,
        block: &str,
        file_path: Option<&Path>,
    ) {
        let entry = section_mut(&mut self.data, "files")
            .entry(rel_path.to_owned())
            .or_insert_with(|| json!({ "blocks": {} }));
        let Some(entry) = entry.as_object_mut() else {
            return;
        };
        if let Some(hash) = blob_hash.filter(|h| !h.is_empty()) {
            entry.insert("blob_hash".to_owned(), json!(hash));
        }
        if let Some(now) = file_path.and_then(stat) {
            entry.insert("mtime_ns".to_owned(), json!(now.mtime_ns));
            entry.insert("size".to_owned(), json!(now.size));
        }
        if let Some(blocks) = entry
            .entry("blocks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
        {
            blocks.insert(py_bool(line_numbers).to_owned(), json!(block));
        }
    }

    pub fn get_summary(
        &mut self,
        rel_path: &str,
        blob_hash: &str,
        file_path: Option<&Path>,
    ) -> Option<String> {
        if let Some(now) = file_path.and_then(stat)
            && let Some(summary) = self
                .files("summaries_meta", rel_path)
                .filter(|entry| matches(entry, &now))
                .and_then(|entry| entry.get("summary"))
                .and_then(Value::as_str)
        {
            let summary = summary.to_owned();
            self.hits += 1;
            return Some(summary);
        }

        let found = self
            .data
            .get("summaries")
            .and_then(|s| s.get(format!("{rel_path}:{blob_hash}")))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if found.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        found
    }

    pub fn set_summary(
        &mut self,
        rel_path: &str,
        blob_hash: &str,
        summary: &str,
        file_path: Option<&Path>,
    ) {
        section_mut(&mut self.data, "summaries")
            .insert(format!("{rel_path}:{blob_hash}"), json!(summary));
        if let Some(now) = file_path.and_then(stat) {
            section_mut(&mut self.data, "summaries_meta").insert(
                rel_path.to_owned(),
                json!({
                    "mtime_ns": now.mtime_ns,
                    "size": now.size,
                    "blob_hash": blob_hash,
                    "summary": summary,
                }),
            );
        }
    }
}

/// `data.setdefault(name, {})`, replacing a value that is not an object.
fn section_mut<'a>(data: &'a mut Map<String, Value>, name: &str) -> &'a mut Map<String, Value> {
    let slot = data.entry(name.to_owned()).or_insert_with(|| json!({}));
    if !slot.is_object() {
        *slot = json!({});
    }
    slot.as_object_mut().expect("just made an object")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_hash_matches_git() {
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(
            compute_git_blob_hash(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        assert_eq!(
            compute_git_blob_hash(b""),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
    }

    fn project() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".codigest")).unwrap();
        let file = dir.path().join("a.py");
        fs::write(&file, "x = 1\n").unwrap();
        (dir, file)
    }

    #[test]
    fn a_block_is_found_again_by_stat_and_by_hash() {
        let (dir, file) = project();
        let mut cache = ContentCache::new(dir.path());
        assert_eq!(cache.get_file_block("a.py", None, false, Some(&file)), None);
        cache.set_file_block("a.py", Some("h1"), false, "BLOCK", Some(&file));
        cache.save();

        let mut again = ContentCache::new(dir.path());
        assert_eq!(
            again
                .get_file_block("a.py", None, false, Some(&file))
                .as_deref(),
            Some("BLOCK")
        );
        // The other numbering was never stored.
        assert_eq!(again.get_file_block("a.py", None, true, Some(&file)), None);
        // Without a usable stat the hash finds it.
        assert_eq!(
            again
                .get_file_block("a.py", Some("h1"), false, None)
                .as_deref(),
            Some("BLOCK")
        );
        assert_eq!(again.get_file_block("a.py", Some("zz"), false, None), None);
        assert_eq!((again.hits, again.misses), (2, 2));
    }

    #[test]
    fn changed_content_misses() {
        let (dir, file) = project();
        let mut cache = ContentCache::new(dir.path());
        cache.set_file_block("a.py", None, false, "OLD", Some(&file));
        fs::write(&file, "x = 22\n").unwrap();
        assert_eq!(cache.get_file_block("a.py", None, false, Some(&file)), None);
    }

    #[test]
    fn save_needs_the_folder_and_keeps_the_format_and_key_order() {
        let (dir, file) = project();
        let mut cache = ContentCache::new(dir.path());
        cache.set_file_block("a.py", Some("h"), true, "B", Some(&file));
        cache.save();
        let text = fs::read_to_string(dir.path().join(".codigest").join("cache.json")).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["format"], 2);
        let keys: Vec<_> = value["files"]["a.py"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["blocks", "blob_hash", "mtime_ns", "size"]);
        assert_eq!(value["files"]["a.py"]["blocks"]["True"], "B");
        assert!(text.starts_with("{\n  \"format\": 2,"));

        let bare = tempfile::tempdir().unwrap();
        ContentCache::new(bare.path()).save();
        assert!(!bare.path().join(".codigest").exists());
    }

    #[test]
    fn another_format_starts_over() {
        let (dir, _file) = project();
        let path = dir.path().join(".codigest").join("cache.json");
        fs::write(&path, r#"{"format": 1, "files": {"a.py": {}}}"#).unwrap();
        let cache = ContentCache::new(dir.path());
        assert!(cache.data.get("files").is_none());
        assert_eq!(cache.data["format"], 2);
    }

    #[test]
    fn summaries_are_kept_by_hash_and_by_stat() {
        let (dir, file) = project();
        let mut cache = ContentCache::new(dir.path());
        cache.set_summary("a.py", "h", "S", Some(&file));
        assert_eq!(
            cache.get_summary("a.py", "other", Some(&file)).as_deref(),
            Some("S")
        );
        assert_eq!(cache.get_summary("a.py", "h", None).as_deref(), Some("S"));
        assert_eq!(cache.get_summary("b.py", "h", None), None);
    }
}
