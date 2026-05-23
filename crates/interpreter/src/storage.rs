use crate::value::{Value, ContentHash, HashAlgorithm, Commit};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use sha2::Digest;

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn init(base_dir: &Path) -> Result<Self> {
        let root = base_dir.join(".oo").join("objects");
        if !root.exists() {
            fs::create_dir_all(&root)?;
        }
        Ok(Self { root })
    }

    pub fn put_value(&self, value: &Value) -> Result<ContentHash> {
        let hash = value.content_hash();
        let content = serde_json::to_string_pretty(value)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    pub fn get_value(&self, hash: &ContentHash) -> Result<Value> {
        let content = self.read_object(hash)?;
        let value: Value = serde_json::from_str(&content)?;
        Ok(value)
    }

    pub fn put_commit(&self, commit: &Commit) -> Result<ContentHash> {
        let hash = commit.content_hash();
        let content = serde_json::to_string_pretty(commit)?;
        self.write_object(&hash, content)?;
        Ok(hash)
    }

    pub fn get_commit(&self, hash: &ContentHash) -> Result<Commit> {
        let content = self.read_object(hash)?;
        let commit: Commit = serde_json::from_str(&content)?;
        Ok(commit)
    }

    pub fn get_head(&self, base_dir: &Path) -> Result<Option<ContentHash>> {
        let head_path = base_dir.join(".oo").join("HEAD");
        if !head_path.exists() { return Ok(None); }
        let s = fs::read_to_string(head_path)?;
        ContentHash::parse(&s.trim()).map(Some).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    pub fn set_head(&self, base_dir: &Path, hash: &ContentHash) -> Result<()> {
        let oo_dir = base_dir.join(".oo");
        if !oo_dir.exists() { fs::create_dir_all(&oo_dir)?; }
        let head_path = oo_dir.join("HEAD");
        fs::write(head_path, hash.to_string())?;
        Ok(())
    }

    pub fn get_horizon_salt(&self) -> ContentHash {
        let mut hasher = sha2::Sha256::new();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        hasher.update(now.to_le_bytes());
        ContentHash { algorithm: HashAlgorithm::Sha256, digest: sha2::Digest::finalize(hasher).to_vec() }
    }

    fn write_object(&self, hash: &ContentHash, content: String) -> Result<()> {
        let path = self.hash_to_path(hash);
        if !path.exists() {
            if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
            fs::write(&path, content)?;
        }
        Ok(())
    }

    fn read_object(&self, hash: &ContentHash) -> Result<String> {
        let path = self.hash_to_path(hash);
        if !path.exists() { return Err(anyhow::anyhow!("Object not found: {}", hash.to_string())); }
        Ok(fs::read_to_string(path)?)
    }

    fn hash_to_path(&self, hash: &ContentHash) -> PathBuf {
        let algo_dir = match hash.algorithm { HashAlgorithm::Sha256 => "sha256" };
        let hex = hex::encode(&hash.digest);
        self.root.join(algo_dir).join(&hex[0..2]).join(&hex[2..])
    }
}
