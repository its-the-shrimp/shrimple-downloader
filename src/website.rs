use {
    crate::{download::{download, Error, MediaKind}, try_harder_async},
    axum::{body::Body, http::Uri, response::IntoResponse},
    http::{header::{CONTENT_DISPOSITION, CONTENT_TYPE}, HeaderMap, StatusCode},
    percent_encoding::percent_decode_str,
};

async fn serve_media(mkind: MediaKind, uri: Uri) -> impl IntoResponse {
    let res: Result<_, Option<&'static str>> = try_harder_async! {
        let mut link = None;
        for pair in uri.query().ok_or("no query parameters provided")?.split('&') {
            link = pair.strip_prefix("link=");
        }
        let link = percent_decode_str(link.ok_or("`link` query parameter missing")?)
            .decode_utf8_lossy();

        log::info!("Downloading {mkind:?} from {link:?}");
        match download(&link, mkind).await {
            Ok(stream) => {
                let name = stream.filename();
                let mime = mkind.mime_type();

                let content_disposition = format!("attachment; filename=\"{name}\"");
                let headers = HeaderMap::from_iter([
                    (CONTENT_TYPE, mime.try_into().map_err(|_| None)?),
                    (CONTENT_DISPOSITION, content_disposition.try_into().map_err(|_| None)?),
                ]);
                (StatusCode::OK, headers, Body::from_stream(stream))
            }
            Err(Error::TooLarge) => Err("The media is too big")?,
            Err(Error::IsStream) => Err("Livestreams can't be downloaded")?,
            Err(Error::NotFound) => {
                Err("Invalid video ID, make sure the link is copied & pasted correctly")?
            }
            Err(Error::InvalidLink) => {
                Err("Invalid link, make sure the link is copied & pasted correctly")?
            }
            Err(Error::DataFetchFailed | Error::MetadataFetchFailed) => {
                Err("Server error")?
            }
        }
    };

    match res {
        Ok(x) => Ok(x),
        Err(Some(msg)) => Err((StatusCode::BAD_REQUEST, msg)),
        Err(None) => {
            log::warn!("String to HeaderValue conversion failed");
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"))
        }
    }
}

pub async fn serve_audio(uri: Uri) -> impl IntoResponse {
    serve_media(MediaKind::Audio, uri).await
}

pub async fn serve_video(uri: Uri) -> impl IntoResponse {
    serve_media(MediaKind::Video, uri).await
}
