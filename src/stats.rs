use std::{
    collections::HashSet,
    fmt::{Display, Formatter},
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, MutexGuard},
};

use axum::{extract::{ConnectInfo, Request, State}, middleware, response::Response};

#[derive(Debug, Default)]
pub struct OwnedStats {
    website_visitors: HashSet<IpAddr>,
    audio_downloaders: HashSet<IpAddr>,
    video_downloaders: HashSet<IpAddr>,
    bot_users: HashSet<u64>,
}

impl Display for OwnedStats {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "website visitors: {}", self.website_visitors.len())?;
        writeln!(f, "audio downloaders: {}", self.audio_downloaders.len())?;
        writeln!(f, "video downloaders: {}", self.video_downloaders.len())?;
        write  !(f, "bot users: {}", self.bot_users.len())?;
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct Stats(Arc<Mutex<OwnedStats>>);

impl Stats {
    #[expect(clippy::expect_used, reason = "nothing better to do if stats are poisoned")]
    pub fn lock(&self) -> MutexGuard<OwnedStats> {
        self.0.lock().expect("failed to get stats")
    }

    pub fn record_website_visitor(&self, addr: IpAddr) {
        self.lock().website_visitors.insert(addr);
    }

    pub fn record_audio_downloader(&self, addr: IpAddr) {
        self.lock().audio_downloaders.insert(addr);
    }

    pub fn record_video_downloader(&self, addr: IpAddr) {
        self.lock().video_downloaders.insert(addr);
    }

    pub fn record_bot_user(&self, id: u64) {
        self.lock().bot_users.insert(id);
    }
}

pub async fn record_website_visitor(
    stats: State<Stats>,
    addr: ConnectInfo<SocketAddr>,
    request: Request,
    next: middleware::Next,
) -> Response {
    stats.record_website_visitor(addr.0.ip());
    next.run(request).await
}

pub async fn record_audio_downloader(
    stats: State<Stats>,
    addr: ConnectInfo<SocketAddr>,
    request: Request,
    next: middleware::Next,
) -> Response {
    stats.record_audio_downloader(addr.0.ip());
    next.run(request).await
}

pub async fn record_video_downloader(
    stats: State<Stats>,
    addr: ConnectInfo<SocketAddr>,
    request: Request,
    next: middleware::Next,
) -> Response {
    stats.record_video_downloader(addr.0.ip());
    next.run(request).await
}
