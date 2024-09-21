pub mod telegram;
mod cache;

use {
    self::cache::Cache, crate::{download, stats::Stats, try_harder_async, utils::{default, Result}}, axum::{extract::State, Json}, futures::{FutureExt, Stream}, log::{logger, set_max_level}, reqwest::{multipart::Part, Body}, std::{io, mem::take, sync::{atomic::{AtomicBool, Ordering::Relaxed}, Arc}}, telegram::{
        DeleteMessage, DeleteWebhook, EditMessageText, GetMe, MediaKind, Message,
        MessageCommon, MessageEntity, MessageEntityKind, MessageKind, SendAudio, SendMessage,
        SendVideo, SetMyCommands, SetWebhook, Update, UpdateKind,
    }, tokio::try_join
};

mod en {
    use {std::{sync::LazyLock, fmt::Write}, super::telegram::BotCommand};

    pub static COMMANDS: [BotCommand; 3] = [
        BotCommand { command: "/help", description: "Show this message" },
        BotCommand { command: "/video", description: "Download a video via the provided link" },
        BotCommand { command: "/audio", description: "Download audio via the provided link" },
    ];

    pub static HELP_MSG: LazyLock<String> = LazyLock::new(|| {
        let mut text = "Available commands:\n\n".to_owned();
        for &BotCommand { command, description } in &COMMANDS {
            _ = writeln!(text, "{command} - {description}");
        }
        text
    });

    pub static LANG_CODE: &str = "en";
}

pub struct Bot {
    username: Box<str>,
    /// Inserted into all videos & tracks sent by the bot.
    caption: Box<str>,
    pub client: telegram::Client,
    pub owner_id: i64,
    pub is_active: AtomicBool,
    stats: Stats,
    cache: Cache,
}

impl Bot {
    async fn new(stats: Stats) -> Result<Self> {
        let client = telegram::Client::default();
        let username = client.request(&GetMe).await?.username
            .ok_or_else(|| io::Error::other("no bot username"))?;
        let res = Self {
            caption: format!("@{username}").into(),
            owner_id: env!("OWNER_TELEGRAM_ID").parse()?,
            cache: Cache::new()?,
            is_active: AtomicBool::new(false),
            stats,
            client,
            username,
        };

        res.client.request(&SetWebhook {
            url: concat!(env!("URL"), "/bot"),
            drop_pending_updates: true,
            secret_token: None, // TODO: add this
        }).await?;
        res.client.request(&SetMyCommands { commands: &en::COMMANDS, language_code: None }).await?;
        res.client.request(&SendMessage { chat_id: res.owner_id, text: "ON", ..default() }).await?;
        res.is_active.store(true, Relaxed);

        Ok(res)
    }

    async fn handle_update(&self, update: &Update) -> Result {
        log::info!("Bot received update: {update:#?}");
        if let Some(id) = update.from().map(|u| u.id) {
            self.stats.record_bot_user(id);
        }

        let UpdateKind::Message(Message { chat, kind, id, .. }) = &update.kind;
        let MessageKind::Common(MessageCommon { media_kind, .. }) = kind;
        let MediaKind::Text { text, entities } = media_kind else {
            return Ok(());
        };
        let [MessageEntity {
            length,
            offset: 0,
            kind: MessageEntityKind::BotCommand,
        }, ..] = &**entities
        else {
            return Ok(());
        };

        let (cmd, args) = text.split_at(*length);
        if let Some(cmd) = cmd.split_once('@')
            .map_or(Some(cmd), |(cmd, dst)| (dst == &*self.username).then_some(cmd))
        {
            self.handle_command(*id, chat.id, cmd, args).await?;
        }

        Ok(())
    }

    async fn handle_command(&self, msg_id: i32, chat_id: i64, cmd: &str, args: &str) -> Result {
        match cmd {
            "/resetstats" if chat_id == self.owner_id => self.handle_resetstats_command(chat_id).await,
            "/stats" if chat_id == self.owner_id => self.handle_stats_command(chat_id).await,
            "/logs" if chat_id == self.owner_id => self.handle_logs_command(),
            "/loglevel" if chat_id == self.owner_id => self.handle_loglevel_command(chat_id, args).await,

            "/help" => self.handle_help_command(chat_id).await,
            "/video" => self.handle_video_command(msg_id, chat_id, args).await,
            "/audio" => self.handle_audio_command(msg_id, chat_id, args).await,
            _ => Ok(()),
        }
    }

    async fn handle_loglevel_command(&self, chat_id: i64, args: &str) -> Result {
        match args.trim().parse() {
            Ok(level) => {
                set_max_level(level);
                self.client.request(&SendMessage {
                    chat_id,
                    text: &format!("Max log level is now {level:?}"),
                    ..default()
                }).await?;
            }
            Err(err) => _ = self.client.request(&SendMessage {
                chat_id,
                text: &format!("Error: {err}"),
                ..default()
            }).await?,
        };
        Ok(())
    }

    #[expect(
        clippy::unused_self,
        clippy::unnecessary_wraps,
        reason = "for symmetry with other command handlers"
    )]
    fn handle_logs_command(&self) -> Result {
        logger().flush();
        Ok(())
    }

    async fn handle_resetstats_command(&self, chat_id: i64) -> Result {
        *self.stats.lock() = default();
        self.handle_stats_command(chat_id).await
    }

    async fn handle_stats_command(&self, chat_id: i64) -> Result {
        let text = { &self.stats.lock().to_string() };
        self.client.request(&SendMessage { chat_id, text, ..default() }).await?;
        Ok(())
    }

    async fn handle_help_command(&self, chat_id: i64) -> Result {
        self.client.request(&SendMessage { chat_id, text: &en::HELP_MSG, ..default() }).await?;
        Ok(())
    }

    async fn handle_video_command(&self, msg_id: i32, chat_id: i64, args: &str) -> Result {
        self.handle_media_command(msg_id, chat_id, args, download::MediaKind::Video).await
    }

    async fn handle_audio_command(&self, msg_id: i32, chat_id: i64, args: &str) -> Result {
        self.handle_media_command(msg_id, chat_id, args, download::MediaKind::Audio).await
    }

    #[expect(clippy::too_many_lines)]
    async fn handle_media_command(
        &self,
        msg_id: i32,
        chat_id: i64,
        args: &str,
        mkind: download::MediaKind,
    ) -> Result {
        let link = args.trim();

        if link.is_empty() {
            self.client.request(&SendMessage {
                chat_id,
                text: match mkind {
                    download::MediaKind::Video => "No link provided\n\
                        An example of using the command:\n\
                        \t/video https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                    download::MediaKind::Audio => "No link provided\n\
                        An example of using the command:\n\
                        \t/audio https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                },
                disable_web_page_preview: true,
                reply_to_message_id: Some(msg_id),
            }).await?;
            return Ok(());
        }

        let Message { id: message_id, .. } = self.client.request(&SendMessage {
            chat_id,
            reply_to_message_id: Some(msg_id),
            text: match mkind {
                download::MediaKind::Video => "Downloading video...",
                download::MediaKind::Audio => "Downloading audio...",
            },
            ..default()
        }).await?;

        #[expect(clippy::significant_drop_in_scrutinee)]
        match try_harder_async! {
            let input = download::Input::from_uri(link).ok_or(Err(download::Error::InvalidLink))?;
            let uri = input.to_string();
            if let Some(cached_id) = self.cache.get(&uri, mkind).await {
                Err(Ok(cached_id))?;
            }
            (uri, download::Media::get(input, mkind).await.map_err(Err)?)
        } {
            Ok((uri, mut stream)) => {
                let stream_size = stream.size_hint().0 as u64;
                let filename = take(stream.filename_mut());
                let payload = Part::stream_with_length(Body::wrap_stream(stream), stream_size)
                    .file_name(filename)
                    .mime_str(mkind.mime_type())?;

                let msg = match mkind {
                    download::MediaKind::Audio => self.client.multipart_request(&SendAudio {
                        chat_id,
                        audio: "attach://payload",
                        caption: &self.caption,
                        reply_to_message_id: Some(msg_id),
                    }, payload).await?,
                    download::MediaKind::Video => self.client.multipart_request(&SendVideo {
                        chat_id,
                        video: "attach://payload",
                        caption: &self.caption,
                        reply_to_message_id: Some(msg_id),
                    }, payload).await?,
                };
                self.client.request(&DeleteMessage { chat_id, message_id }).await?;

                let MessageKind::Common(msg) = msg.kind;
                let tg_id = match (msg.media_kind, mkind) {
                    (MediaKind::Audio { audio }, download::MediaKind::Audio) => audio.id,
                    (MediaKind::Video { video }, download::MediaKind::Video) => video.id,
                    _ => Err(io::Error::other("unexpected media kind"))?,
                };
                self.cache.set(uri.into(), mkind, tg_id).await;
            }

            Err(Ok(cached_id)) => _ = try_join! {
                match mkind {
                    download::MediaKind::Audio => self.client.request(&SendAudio {
                        chat_id,
                        audio: &cached_id,
                        caption: &self.caption,
                        reply_to_message_id: Some(msg_id),
                    }).left_future(),
                    download::MediaKind::Video => self.client.request(&SendVideo {
                        chat_id,
                        video: &cached_id,
                        caption: &self.caption,
                        reply_to_message_id: Some(msg_id),
                    }).right_future(),
                },
                self.client.request(&DeleteMessage { chat_id, message_id }),
            }?,

            Err(Err(download::Error::TooLarge)) => {
                self.client.request(&EditMessageText {
                    chat_id,
                    message_id,
                    text: match mkind {
                        download::MediaKind::Video => "The video is too large",
                        download::MediaKind::Audio => "The track is too large",
                    },
                }).await?;
            }

            Err(Err(download::Error::IsStream)) => {
                self.client.request(&EditMessageText {
                    chat_id,
                    message_id,
                    text: "Live streams can't be downloaded while they're ongoing",
                }).await?;
            }

            Err(Err(download::Error::NotFound | download::Error::InvalidLink)) => {
                self.client.request(&EditMessageText {
                    chat_id,
                    message_id,
                    text: "The provided link doesn't point to an existing video/track.\n\
                           Make sure the link is copied correctly and try again.\n\
                           Keep in mind that shortened links are not accepted."
                }).await?;
            }

            Err(Err(download::Error::DataFetchFailed | download::Error::MetadataFetchFailed)) => {
                self.client.request(&EditMessageText {
                    chat_id,
                    message_id,
                    text: "An unexpected error occured while downloading",
                }).await?;
            }
        }

        Ok(())
    }
}

pub async fn init(stats: Stats) -> Result<Arc<Bot>> {
    Bot::new(stats).await.map(Arc::new)
}

pub async fn deinit(bot: Arc<Bot>) -> Result {
    try_join! {
        bot.cache.sync(),
        bot.client.request(&DeleteWebhook),
        bot.client.request(&SendMessage { chat_id: bot.owner_id, text: "OFF", ..default() }),
    }?;
    bot.is_active.store(false, Relaxed);
    if Arc::strong_count(&bot) > 1 {
        log::warn!("Bot::deinit: Something else is still using the Bot instance");
    }
    Ok(())
}

pub async fn handle_update(state: State<Arc<Bot>>, update: Json<Update>) {
    if let Err(err) = state.handle_update(&update).await {
        log::error!("Telegram bot error\nUpdate: {update:#?}\nError: {err}");
    }
}
