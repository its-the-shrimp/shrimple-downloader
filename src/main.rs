mod bot;
mod website;
mod download;
mod utils;
mod stats;
mod logger;

use {
    axum::{middleware, routing::{get, post}, serve},
    futures::TryFutureExt,
    stats::{record_audio_downloader, record_video_downloader, record_website_visitor, Stats},
    std::net::{Ipv4Addr, SocketAddr},
    tokio::{net::TcpListener, signal::ctrl_c},
    tower::ServiceBuilder,
    tower_http::services::ServeDir,
    utils::Result,
};

#[tokio::main]
async fn main() -> Result {
    let stats = Stats::default();
    let bot = bot::init(stats.clone()).await?;
    logger::init(bot.clone())?;
    
    let router = axum::Router::new()
        .route("/bot", post(bot::handle_update))
        .route("/video", get(website::serve_video)
            .layer(middleware::from_fn_with_state(stats.clone(), record_video_downloader)))
        .route("/audio", get(website::serve_audio)
            .layer(middleware::from_fn_with_state(stats.clone(), record_audio_downloader)))
        .fallback_service(ServiceBuilder::new()
            .layer(middleware::from_fn_with_state(stats.clone(), record_website_visitor)) 
            .service(ServeDir::new("dist")
                .not_found_service(serve_embedded_html!("../dist/404.html"))))
        .with_state(bot.clone())
        .into_make_service_with_connect_info::<SocketAddr>();

    log::info!("Starting a server on {}", env!("URL"));
    serve(TcpListener::bind(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 8443)).await?, router)
        .with_graceful_shutdown(ctrl_c().unwrap_or_else(drop))
        .await?;
    bot::deinit(bot).await?;
    logger::deinit();

    Ok(())
}
