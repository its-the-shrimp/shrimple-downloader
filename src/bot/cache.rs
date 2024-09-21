use {
    crate::{download::MediaKind, utils::{default, Result}},
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, fs::File, io::ErrorKind::NotFound, ops::Deref},
    tokio::sync::{RwLock, RwLockReadGuard},
};

const CACHE_PATH: &str = concat!(env!("CACHE_DIR"), "tg_id_cache.json");

#[derive(Deserialize, Serialize, Default)]
struct Inner {
    /// Maps links to audio files to their Telegram IDs.
    tracks: HashMap<Box<str>, Box<str>>,
    /// Maps links to video files to their Telegram IDs.
    videos: HashMap<Box<str>, Box<str>>,
}

pub struct Cache(RwLock<Inner>);

impl Cache {
    pub fn new() -> Result<Self> {
        let inner = match File::open(CACHE_PATH) {
            Ok(file) => serde_json::from_reader(file)?,
            Err(e) if e.kind() == NotFound => default(),
            Err(e) => Err(e)?,
        };
        Ok(Self(RwLock::new(inner)))
    }

    pub async fn get(&self, uri: &str, mkind: MediaKind) -> Option<RwLockReadGuard<str>> {
        RwLockReadGuard::try_map(self.0.read().await, |inner| match mkind {
            MediaKind::Video => inner.videos.get(uri),
            MediaKind::Audio => inner.tracks.get(uri),
        }.map(Deref::deref)).ok()
    }

    pub async fn set(&self, uri: Box<str>, mkind: MediaKind, tg_id: Box<str>) {
        let mut inner = self.0.write().await;
        match mkind {
            MediaKind::Video => inner.videos.insert(uri, tg_id),
            MediaKind::Audio => inner.tracks.insert(uri, tg_id),
        };
    }

    pub async fn sync(&self) -> Result {
        serde_json::to_writer(File::create(CACHE_PATH)?, &*self.0.read().await)?;
        Ok(())
    }
}
