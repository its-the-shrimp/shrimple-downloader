use {
    crate::{bot::{telegram::{SendMessage, MAX_MSG_LEN}, Bot}, utils::{default, LimitedFormatter}},
    futures::executor::block_on,
    log::{logger, max_level, set_boxed_logger, set_max_level, LevelFilter, Log, SetLoggerError},
    std::{
        fmt::Write as _,
        io::{Cursor, Write as _},
        iter,
        str::from_utf8_unchecked,
        sync::{atomic::Ordering::Relaxed, Arc, Mutex, MutexGuard},
    },
    tokio::spawn,
};

struct Logger {
    /// All records must be less than [`MAX_MSG_LEN`]
    records: Arc<Mutex<heapless::Deque<Box<str>, 100>>>,
    bot: Arc<Bot>,
}

fn get_records(unlocked: &Mutex<heapless::Deque<Box<str>, 100>>)
    -> MutexGuard<heapless::Deque<Box<str>, 100>>
{
    const ERR_MSG: &str = "logs forcibly cleared because a thread panicked while logging";

    unlocked.lock().unwrap_or_else(|e| {
        let mut records = e.into_inner();
        *records = default();
        _ = records.push_back(ERR_MSG.into());
        records
    })
}

impl Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let level = max_level();
        if level < record.level() {
            return;
        }
        set_max_level(LevelFilter::Off);

        let mut records = get_records(&self.records);
        let mut buf = LimitedFormatter::<MAX_MSG_LEN>::new();
        _ = write!(buf, "{}: {}", record.module_path().unwrap_or(""), record.args());
        let new = Box::from(buf.as_str());

        if records.is_empty() {
            spawn(self.bot.client.request(&SendMessage {
                chat_id: self.bot.owner_id, 
                text: "New logs available",
                ..default()
            }));
        }

        unsafe {
            if records.is_full() {
                _ = records.pop_front_unchecked();
            }
            records.push_back_unchecked(new);
        }
        drop(records);

        set_max_level(level);
    }

    fn flush(&self) {
        let level = max_level();
        set_max_level(LevelFilter::Off);

        let records = Arc::clone(&self.records);
        let bot = Arc::clone(&self.bot);
        let sender = async move {
            let mut buf = Cursor::new([0u8; MAX_MSG_LEN]);
            for record in iter::from_fn(|| get_records(&records).pop_front()) {
                let pos = buf.position();
                let rem = MAX_MSG_LEN as u64 - pos;
                if record.len() as u64 > rem {
                    let text = unsafe {
                        from_utf8_unchecked(&buf.get_ref()[..pos.try_into().unwrap_or(usize::MAX)])
                    };
                    _ = bot.client.request(&SendMessage {
                        chat_id: bot.owner_id,
                        text,
                        ..default()
                    }).await;
                    buf.set_position(0);
                }

                _ = buf.write(record.as_bytes());
                _ = buf.write(b"\n\n");
            }

            if let pos @ 1.. = buf.position() {
                let text = unsafe {
                    from_utf8_unchecked(&buf.get_ref()[..pos.try_into().unwrap_or(usize::MAX)])
                };
                _ = bot.client.request(&SendMessage {
                    chat_id: bot.owner_id,
                    text,
                    ..default()
                }).await;
            } else {
                _ = bot.client.request(&SendMessage {
                    chat_id: bot.owner_id,
                    text: "No logs available",
                    ..default()
                }).await;
            }

            set_max_level(level);
        };

        if self.bot.is_active.load(Relaxed) {
            tokio::spawn(sender);
        } else {
            block_on(sender);
        }
    }
}

pub fn init(bot: Arc<Bot>) -> Result<(), SetLoggerError> {
    set_max_level(LevelFilter::Warn);
    set_boxed_logger(Box::new(Logger { records: default(), bot }))
}

pub fn deinit() {
    logger().flush();
}
