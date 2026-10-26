use crate::settings::Typography;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub version: u32,
    pub typography: Typography,
    pub spread: bool,
    pub theme: u8,
    pub page_color: u8,
    pub books: BTreeMap<String, BookState>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookState {
    pub path: PathBuf,
    pub anchor: u64,
    #[serde(default)]
    pub encoding: Option<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            typography: Typography::default(),
            spread: false,
            theme: 0,
            page_color: 0,
            books: BTreeMap::new(),
        }
    }
}
impl Preferences {
    pub fn load(path: &Path) -> Self {
        let mut value = fs::read(path)
            .ok()
            .filter(|b| b.len() < 1024 * 1024)
            .and_then(|b| serde_json::from_slice::<Self>(&b).ok())
            .filter(|s| s.version == 1)
            .unwrap_or_default();
        value.typography.validate();
        value.theme = value.theme.min(2);
        value.page_color = value.page_color.min(2);
        value
    }
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)?;
        Ok(())
    }
}
pub fn config_path(portable: bool) -> PathBuf {
    if let Some(path) = std::env::var_os("SAE_READER_DATA_DIR") {
        return PathBuf::from(path).join("state.json");
    }
    if portable {
        return std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(Path::new("."))
            .join("data/state.json");
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("SaeReader/state.json")
}
pub fn fingerprint(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hash = blake3::Hasher::new();
    hash.update(&size.to_le_bytes());
    let mut buffer = vec![0u8; 65536];
    let read = file.read(&mut buffer)?;
    hash.update(&buffer[..read]);
    if size > 65536 {
        file.seek(SeekFrom::End(-65536))?;
        let read = file.read(&mut buffer)?;
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize().to_hex().to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replace_existing_settings_atomically() {
        let folder =
            std::env::temp_dir().join(format!("sae-reader-settings-{}", std::process::id()));
        let path = folder.join("state.json");
        let mut settings = Preferences::default();
        settings.save(&path).unwrap();
        settings.spread = true;
        settings.save(&path).unwrap();
        assert!(Preferences::load(&path).spread);
        fs::write(&path, b"{broken").unwrap();
        assert!(!Preferences::load(&path).spread);
        fs::remove_file(path).unwrap();
        fs::remove_dir(folder).unwrap();
    }
}
