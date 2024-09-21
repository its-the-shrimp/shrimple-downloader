pub type Result<T = (), E = Box<dyn std::error::Error + Send + Sync>> = std::result::Result<T, E>;

/// `try { }` blocks in stable Rust
#[macro_export]
macro_rules! try_harder {
    ( $($body:tt)* ) => (
        (|| {
            let output = { $($body)* };
            ::core::iter::empty()
                .try_fold(output, |_, it: ::core::convert::Infallible| match it {})
        })()
    )
}

/// `try { }` blocks in stable Rust, for use in an async context
#[macro_export]
macro_rules! try_harder_async {
    ( $($body:tt)* ) => (
        (|| async {
            let output = { $($body)* };
            ::core::iter::empty()
                .try_fold(output, |_, it: ::core::convert::Infallible| match it {})
        })().await
    )
}

pub fn default<T: Default>() -> T {
    T::default()
}

/// For formatting a value while limiting the resulting string to N bytes in length
/// Unlike writing into a `heapless::String` or a `Cursor<[u8; CAP]>`, this object doesn't report
/// an error if a string overflows its buffer
///
/// Another difference is that if the formatter's buffer is exhausted, the string received from 
/// [`LimitedFormatter::to_string`] will have its last 3 chars replaced with "..."
#[derive(Clone, Copy)]
pub struct LimitedFormatter<const CAP: usize> {
    buf: [u8; CAP],
    len: usize,
}

impl<const CAP: usize> std::fmt::Write for LimitedFormatter<CAP> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let rem = CAP - self.len;
        let mut overflow = false;
        let s = s.get(..rem)
            .inspect(|_| overflow = true)
            .unwrap_or(s);
        self.buf[self.len .. self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        if overflow {
            self.buf[CAP - 3 ..].copy_from_slice(b"...");
        }
        Ok(())
    }
}

impl<const CAP: usize> LimitedFormatter<CAP> {
    pub const fn new() -> Self {
        Self { buf: [0u8; CAP], len: 0 }
    }

    /// The string is guaranteed to be at most `CAP` bytes long
    pub const fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.buf) }
    }
}

#[macro_export]
macro_rules! serve_embedded_html {
    ($path:literal) => {{
        let content = std::include_bytes!($path);
        let mime_type = const { http::HeaderValue::from_static("text/html; charset=utf-8") };
        let headers = http::HeaderMap::from_iter([(http::header::CONTENT_TYPE, mime_type)]);
        tower::service_fn(move |_| {
            let body = http_body_util::Full::new(axum::body::Bytes::from_static(content));
            let mut res = axum::response::Response::new(body);
            *res.headers_mut() = headers.clone();
            std::future::ready(Ok(res))
        })
    }};
}
