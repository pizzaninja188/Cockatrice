//! Versioned, verified last-good cache. Live games keep their admitted snapshot.
use crate::format_catalog::Catalog;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub hash: String,
    pub source_date: String,
    pub checked_at: u64,
}
#[derive(Clone)]
pub struct Snapshot {
    pub catalog: Arc<Catalog>,
    pub checked_at: u64,
}
impl Snapshot {
    pub fn stale(&self) -> bool {
        now().saturating_sub(self.checked_at) > 48 * 60 * 60
    }
}
static CURRENT: OnceLock<RwLock<Option<Snapshot>>> = OnceLock::new();
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn cache_dir() -> PathBuf {
    std::env::var_os("TRICERULES_FORMAT_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
                .unwrap_or_else(std::env::temp_dir)
                .join("Cockatrice")
                .join("tricerules")
                .join("format-data")
        })
}
pub fn read_cache(dir: &Path) -> Result<Snapshot, String> {
    let manifest: Manifest = serde_json::from_slice(
        &std::fs::read(dir.join("current.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest.hash.len() != 64 || !manifest.hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid format cache identity".into());
    }
    let path = dir.join(format!("{}.jsonl.gz", manifest.hash));
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("Format cache is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(Snapshot {
        catalog: Arc::new(Catalog::from_gzip(
            &bytes,
            &manifest.hash,
            manifest.source_date,
        )?),
        checked_at: manifest.checked_at,
    })
}
pub fn current() -> Option<Snapshot> {
    let state = CURRENT.get_or_init(|| RwLock::new(read_cache(&cache_dir()).ok()));
    let existing = state.read().ok()?.clone();
    // The administrator refresh command promotes this same manifest. Queries pick up a new
    // version immediately; existing games never call this function after admission.
    let manifest = std::fs::read(cache_dir().join("current.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).ok());
    if let Some(manifest) = manifest {
        if existing
            .as_ref()
            .is_none_or(|s| s.catalog.hash != manifest.hash || s.checked_at != manifest.checked_at)
        {
            if let Ok(reloaded) = read_cache(&cache_dir()) {
                if let Ok(mut current) = state.write() {
                    *current = Some(reloaded.clone());
                }
                return Some(reloaded);
            }
        }
    }
    existing
}

fn promote(dir: &Path, bytes: &[u8], manifest: &Manifest) -> Result<Snapshot, String> {
    let catalog = Catalog::from_gzip(bytes, &manifest.hash, manifest.source_date.clone())?;
    if catalog.cards.len() < 10_000
        || ["Plains", "Island", "Swamp", "Mountain", "Forest"]
            .iter()
            .any(|n| catalog.find(n).is_none())
    {
        return Err("Incomplete format catalogue".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let source = dir.join(format!("{}.jsonl.gz", manifest.hash));
    let source_temporary = dir.join(format!("{}.{}.tmp", manifest.hash, std::process::id()));
    std::fs::write(&source_temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(source_temporary, &source).map_err(|e| e.to_string())?;
    let temporary = dir.join(format!("current.{}.tmp", std::process::id()));
    std::fs::write(
        &temporary,
        serde_json::to_vec(manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, dir.join("current.json")).map_err(|e| e.to_string())?;
    Ok(Snapshot {
        catalog: Arc::new(catalog),
        checked_at: manifest.checked_at,
    })
}
pub fn refresh(dir: &Path) -> Result<Snapshot, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(
            "Cockatrice-tricerules-formats/1.0 (https://github.com/pizzaninja188/Cockatrice)",
        )
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let index: serde_json::Value = client
        .get("https://api.scryfall.com/bulk-data")
        .header("Accept", "application/json")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let entry = index["data"]
        .as_array()
        .and_then(|a| a.iter().find(|r| r["type"] == "oracle_cards"))
        .ok_or("Scryfall oracle catalogue is missing")?;
    let uri = entry["jsonl_download_uri"]
        .as_str()
        .ok_or("Scryfall JSONL source is missing")?;
    let url = reqwest::Url::parse(uri).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("data.scryfall.io")
        || !url.path().ends_with(".jsonl.gz")
    {
        return Err("Untrusted format source URL".into());
    }
    let date = entry["updated_at"]
        .as_str()
        .ok_or("Format source date is missing")?
        .to_owned();
    // A successful check of the same source renews freshness without redownloading it.
    if let Ok(existing) = read_cache(dir) {
        if existing.catalog.source_date == date {
            let manifest = Manifest {
                hash: existing.catalog.hash.clone(),
                source_date: date,
                checked_at: now(),
            };
            let bytes = std::fs::read(dir.join(format!("{}.jsonl.gz", manifest.hash)))
                .map_err(|e| e.to_string())?;
            return promote(dir, &bytes, &manifest);
        }
    }
    let mut bytes = Vec::new();
    client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("Format download exceeds limit".into());
    }
    let manifest = Manifest {
        hash: format!("{:x}", Sha256::digest(&bytes)),
        source_date: date,
        checked_at: now(),
    };
    promote(dir, &bytes, &manifest)
}
/// Fetching happens outside the async executor and never blocks an active game.
pub fn start_updates() {
    let _ = current();
    if std::env::var("TRICERULES_FORMAT_UPDATES").as_deref() == Ok("0") {
        eprintln!("tricerules: automatic format updates disabled; using verified cache only");
        return;
    }
    std::thread::spawn(|| loop {
        match refresh(&cache_dir()) {
            Ok(snapshot) => {
                if let Ok(mut state) = CURRENT.get_or_init(|| RwLock::new(None)).write() {
                    *state = Some(snapshot);
                }
            }
            Err(error) => eprintln!(
                "tricerules: format refresh failed; retaining last good catalogue: {error}"
            ),
        }
        std::thread::sleep(INTERVAL);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_cache_and_failed_promotion_keep_the_previous_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("current.json"), b"previous").unwrap();
        let manifest = Manifest {
            hash: "0".repeat(64),
            source_date: "date".into(),
            checked_at: now(),
        };
        assert!(read_cache(dir.path()).is_err());
        assert!(promote(dir.path(), b"invalid gzip", &manifest).is_err());
        assert_eq!(
            std::fs::read(dir.path().join("current.json")).unwrap(),
            b"previous"
        );
        std::fs::write(
            dir.path().join("current.json"),
            serde_json::to_vec(&Manifest {
                hash: "../outside".into(),
                ..manifest
            })
            .unwrap(),
        )
        .unwrap();
        assert!(read_cache(dir.path()).is_err());
    }
}
