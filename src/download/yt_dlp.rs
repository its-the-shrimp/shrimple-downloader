use {
    super::{Error, MediaKind, CACHE_DIR, MAX_FILESIZE},
    crate::utils::Result,
    axum::body::Bytes,
    futures::{Stream, StreamExt},
    serde::Deserialize,
    std::{borrow::Cow, pin::Pin, process::{Output, Stdio}, task::{Context, Poll}},
    tokio::{fs, io::AsyncReadExt, process::{ChildStdout, Command}},
    tokio_util::io::ReaderStream,
};

#[derive(Debug, Deserialize)]
struct MediaData<'src> {
    id: Cow<'src, str>,
    format_id: Cow<'src, str>,
    title: Cow<'src, str>,
    filesize: Option<usize>,
    #[serde(default)]
    is_live: bool,
}

pub struct Media {
    inner: Result<ReaderStream<ChildStdout>, Option<Bytes>>,
    filesize: usize,
    pub filename: String,
}

impl Stream for Media {
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match &mut self.inner {
            Ok(stream) => stream.poll_next_unpin(cx).map_err(Into::into),
            Err(bytes) => bytes.take().map(Ok).into(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.filesize, Some(self.filesize))
    }
}

impl Media {
    pub async fn get(uri: String, mkind: MediaKind) -> Result<Self, Error> {
        let mut cmd = Command::new("yt-dlp");
        let bytes = match cmd
            .args((mkind == MediaKind::Audio).then_some("-x"))
            .args(["--no-download", "-J", &uri])
            .output().await
        {
            Ok(Output { status, stderr, stdout }) => if status.success() {
                stdout
            } else {
                return Err(if stderr.ends_with(b"truncated.\n") {
                    Error::NotFound
                } else {
                    log::error!("`yt-dlp` exited unsuccessfully while downloading the video:\n\
                                command: {cmd:?}\n\
                                stderr:\n{}",
                                String::from_utf8_lossy(&stderr));
                    Error::MetadataFetchFailed
                })
            }
            Err(err) => {
                log::error!("failed to launch `yt-dlp` to get the video data: {err}");
                return Err(Error::MetadataFetchFailed);
            }
        };

        let MediaData { id, format_id, title, filesize, is_live } = serde_json::from_slice(&bytes)
            .map_err(|err| {
                log::error!("failed to decode video data as JSON: {err}");
                Error::MetadataFetchFailed
            })?;

        #[cfg(debug_assertions)]
        fs::write(format!("{CACHE_DIR}{id}.json"), &bytes).await
            .map_err(|_| Error::MetadataFetchFailed)?;

        if is_live {
            return Err(Error::IsStream);
        }

        let mut cmd = Command::new("yt-dlp");
        cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .args(match mkind {
                MediaKind::Video => &["--recode-video", "mp4"][..],
                MediaKind::Audio => &["--audio-format", "mp3", "-x"][..],
            })
            .args([
                "-f", &format_id,
                "--embed-metadata",
                "--embed-thumbnail",
                "-o", "-",
                &uri,
            ]);
        let mut yt_dlp = cmd.spawn().map_err(|e| {
            log::error!("Failed to download media\ncommand: {cmd:#?}\ncause: {e}");
            Error::DataFetchFailed
        })?;
        let mut stdout = yt_dlp.stdout.take().ok_or_else(|| {
            log::error!("Failed to get the stdout of `yt-dlp`");
            Error::DataFetchFailed
        })?;

        let filename = format!("{title}.{}", mkind.extension());
        Ok(if let Some(filesize) = filesize {
            if filesize >= MAX_FILESIZE {
                return Err(Error::TooLarge);
            }
            Self { inner: Ok(ReaderStream::new(stdout)), filename, filesize }
        } else {
            let mut bytes = vec![];
            let filesize = stdout.read_to_end(&mut bytes).await
                .map_err(|_| Error::DataFetchFailed)?;
            Self { inner: Err(Some(bytes.into())), filesize, filename }
        })
    }
}
