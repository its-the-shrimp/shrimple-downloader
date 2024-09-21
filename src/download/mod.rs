mod yt_dlp;
//mod piped;

use {
    crate::utils::Result,
    axum::{body::Bytes, http::Uri},
    futures::{Stream, StreamExt},
    std::{fmt::{Display, Formatter}, pin::Pin, str::FromStr, task::{Context, Poll}},
};

pub const CACHE_DIR: &str = env!("CACHE_DIR");
pub const MAX_FILESIZE: usize = 1 << 30; // 1GB

pub enum Input {
    //Piped { id: String },
    YtDlp { uri: String },
}

impl Input {
    pub fn from_uri(uri: &str) -> Option<Self> {
        let uri = Uri::from_str(uri).ok()?;
        let (path, query) = uri.path_and_query().map(|x| (x.path(), x.query()))?;
        match uri.host()? {
            "www.youtube.com" | "music.youtube.com" if path == "/watch" => query
                .unwrap_or_default()
                .split('&')
                .find_map(|pair| pair.strip_prefix("v="))
                .map(|id| Self::YtDlp { uri: format!("https://youtu.be/{id}") }),
            "youtu.be" => path
                .strip_prefix('/')
                .map(|id| Self::YtDlp { uri: format!("https://youtu.be/{id}") }),
            "www.instagram.com" => path
                .strip_prefix("/reel/")
                .map(|id| Self::YtDlp { uri: format!("https://www.instagram.com/reel/{id}") }),
            host @(
                | "vm.tiktok.com"
                | "vk.com"
                | "twitter.com"
                | "x.com"
            ) => Some(Self::YtDlp { uri: format!("https://{host}{path}") }),
            _ => None,
        }
    }
}

impl Display for Input {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            //Self::Piped { id } => write!(f, "https://youtu.be/{id}"),
            Self::YtDlp { uri } => f.write_str(uri),
        }
    }
}

pub enum Error {
    /// The requested audio/video couldn't be found.
    NotFound,
    /// The requested audio/video is a live stream and doesn't have a defined end.
    IsStream,
    /// The requested audio/video is too long.
    TooLarge,
    /// The error is related to the metadata of the video.
    MetadataFetchFailed,
    /// The error is related to the audio/video data itself.
    DataFetchFailed,
    /// The provided link is either malformed or doesn't lead to a downloadable video/audio
    InvalidLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
}

impl MediaKind {
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Video => "video/mpeg",
            Self::Audio => "audio/mpeg",
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Video => "mp4",
            Self::Audio => "mp3",
        }
    }
}

pub enum Media {
    YtDlp(yt_dlp::Media),
    //Piped(piped::Media),
}

impl Stream for Media {
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match &mut *self {
            Self::YtDlp(media) => media.poll_next_unpin(cx),
            //Self::Piped(media) => media.poll_next_unpin(cx),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::YtDlp(media) => media.size_hint(),
            //Self::Piped(media) => media.size_hint(),
        }
    }
}

impl Media {
    pub async fn get(input: Input, mkind: MediaKind) -> Result<Self, Error> {
        match input {
            Input::YtDlp { uri } => yt_dlp::Media::get(uri, mkind).await.map(Self::YtDlp),
            //Input::Piped { id } => piped::Media::get(id, mkind).await.map(Self::Piped),
        }
    }

    pub fn filename(&self) -> &str {
        match self {
            Self::YtDlp(media) => &media.filename,
            //Self::Piped(media) => &media.filename,
        }
    }

    pub fn filename_mut(&mut self) -> &mut String {
        match self {
            Self::YtDlp(media) => &mut media.filename,
            //Self::Piped(media) => &mut media.filename,
        }
    }
}

pub async fn download(uri: &str, mkind: MediaKind) -> Result<Media, Error> {
    let input = Input::from_uri(uri).ok_or(Error::InvalidLink)?;
    Media::get(input, mkind).await
}
