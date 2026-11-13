//! Append-only JSONL log of finished sessions, indexed for resume.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize, de::DeserializeOwned, de::IgnoredAny};
use sha2::{Digest, Sha256};

use crate::session::{Session, SessionKey};

/// Hex sha256 of model id, budget and task; changes when any changes.
/// Own copy, not `statix::strings::hash_text`: statix pulls in tree-sitter.
pub fn prompt_hash(model_id: &str, budget: usize, task: &str) -> String {
    Sha256::digest(format!("{model_id}\0{budget}\0{task}"))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub struct Log {
    inner: Mutex<Inner>,
}

/// (stage, subject, prompt hash).
type Key = (String, String, String);

struct Inner {
    file: File,
    /// Key -> raw line; later lines win.
    // ponytail: whole log held in RAM; store byte offsets if logs grow large.
    index: HashMap<Key, String>,
}

/// `S` is `Session<A>` when read, `&Session<A>` when written, `IgnoredAny` when indexing.
#[derive(Serialize, Deserialize)]
struct Record<S> {
    stage: String,
    subject: String,
    prompt_hash: String,
    #[serde(flatten)]
    session: S,
}

impl Log {
    /// Opens or creates `path`. Malformed lines (e.g. a crash mid-write) are skipped.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(path)?;
        let text = read_all(&mut file)?;
        // A half-written last line must not swallow the next record.
        if !text.is_empty() && !text.ends_with('\n') {
            file.write_all(b"\n")?;
        }
        let index = index_lines(&text);
        Ok(Self {
            inner: Mutex::new(Inner { file, index }),
        })
    }

    /// A line that no longer parses as `A` counts as a miss.
    pub(crate) fn lookup<A: DeserializeOwned>(
        &self,
        key: &SessionKey,
        hash: &str,
    ) -> Option<Session<A>> {
        let inner = self.lock();
        let line = inner.index.get(&key_of(key, hash))?;
        serde_json::from_str::<Record<Session<A>>>(line)
            .ok()
            .map(|r| r.session)
    }

    /// One `write_all` per record, so a record and its newline never split.
    pub(crate) fn append<A: Serialize>(
        &self,
        key: &SessionKey,
        hash: &str,
        session: &Session<A>,
    ) -> io::Result<()> {
        let record = Record {
            stage: key.stage.clone(),
            subject: key.subject.clone(),
            prompt_hash: hash.to_string(),
            session,
        };
        let line = serde_json::to_string(&record)?;
        let mut inner = self.lock();
        inner.file.write_all(format!("{line}\n").as_bytes())?;
        inner.index.insert(key_of(key, hash), line);
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn key_of(key: &SessionKey, hash: &str) -> Key {
    (key.stage.clone(), key.subject.clone(), hash.to_string())
}

/// Lossy, so a torn multi-byte char only breaks its own line.
fn read_all(file: &mut File) -> io::Result<String> {
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0))?;
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn index_lines(text: &str) -> HashMap<Key, String> {
    let mut index = HashMap::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Record<IgnoredAny>>(line) {
            Ok(r) => {
                index.insert((r.stage, r.subject, r.prompt_hash), line.to_string());
            }
            Err(e) => eprintln!("agent-loop: skipping log line {}: {e}", n + 1),
        }
    }
    index
}
