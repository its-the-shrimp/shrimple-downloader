use {
    super::{Error, MediaKind},
    crate::utils::Result,
    axum::body::Bytes,
    futures::{Stream, StreamExt, TryFutureExt},
    serde::Deserialize,
    std::{pin::Pin, task::{Context, Poll}},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaStream {
    bitrate: u32,
    content_length: usize,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaData {
    title: String,
    audio_streams: Vec<MediaStream>,
    video_streams: Vec<MediaStream>,
}

type BytesStream = impl Stream<Item = reqwest::Result<Bytes>> + Unpin;

pub struct Media {
    inner: BytesStream,
    content_length: usize,
    pub filename: String,
}

impl Stream for Media {
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx).map_err(Into::into)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.content_length, Some(self.content_length))
    }
}

impl Media {
    const X: usize = std::mem::size_of::<BytesStream>();

    pub async fn get(mut id: String, mkind: MediaKind) -> Result<Self, Error> {
        id.insert_str(0, "https://pipedapi.r4fo.com/streams/");
        let MediaData { mut title, audio_streams, video_streams } = reqwest::get(id)
            .and_then(reqwest::Response::json)
            .await
            .map_err(|e| {
                log::error!("Failed to extract metadata from Piped API: {e}");
                Error::MetadataFetchFailed
            })?;

        let stream = match mkind {
            MediaKind::Video => video_streams
                .iter()
                .max_by_key(|&stream| stream.bitrate)
                .ok_or_else(|| {
                    log::error!("No video streams found");
                    Error::MetadataFetchFailed
                })?,
            MediaKind::Audio => audio_streams
                .iter()
                .max_by_key(|&stream| stream.bitrate)
                .ok_or_else(|| {
                    log::error!("No audio streams found");
                    Error::MetadataFetchFailed
                })?,
        };

        let inner: BytesStream = reqwest::get(&stream.url)
            .map_ok(reqwest::Response::bytes_stream)
            .await
            .map_err(|e| {
                log::error!("Failed to download the video: {e}");
                Error::DataFetchFailed
            })?;

        title.push('.');
        title.push_str(mkind.extension());
        Ok(Self { inner, content_length: stream.content_length, filename: title })
    }
}
