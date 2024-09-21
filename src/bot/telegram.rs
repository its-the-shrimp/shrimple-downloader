use {
    crate::utils::Result,
    reqwest::multipart::{Form, Part},
    serde::{de::DeserializeOwned, ser::{Impossible, SerializeStruct}, Deserialize, Serialize, Serializer},
    std::{fmt::Debug, future::Future},
};

pub const MAX_MSG_LEN: usize = 4096;

pub trait Request: Serialize {
    const NAME: &str;
    const URL: &str;
    type Response: DeserializeOwned;
}

#[derive(Debug, Serialize)]
pub struct SetWebhook<'url, 'secret_token> {
    pub url: &'url str,
    pub drop_pending_updates: bool,
    pub secret_token: Option<&'secret_token str>,
}

#[derive(Debug, Serialize)]
pub struct BotCommand {
    pub command: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SetMyCommands<'commands, 'language_code> {
    pub commands: &'commands [BotCommand],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_code: Option<&'language_code str>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    #[serde(rename = "update_id")]
    pub id: u64,
    #[serde(flatten)]
    pub kind: UpdateKind,
}

impl Update {
    pub const fn from(&self) -> Option<&User> {
        match &self.kind {
            UpdateKind::Message(m) => m.from.as_ref(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKind {
    Message(Message),
}

#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(rename = "message_id")]
    pub id: i32,
    pub from: Option<User>,
    pub chat: Chat,
    #[serde(flatten)]
    pub kind: MessageKind,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MessageKind {
    Common(MessageCommon),
}

#[derive(Debug, Deserialize)]
pub struct MessageCommon {
    pub from: Option<User>,
    pub sender_chat: Option<Chat>,
    #[serde(flatten)]
    pub media_kind: MediaKind,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MediaKind {
    Text {
        text: Box<str>,
        #[serde(default)]
        entities: Box<[MessageEntity]>,
    },
    Audio {
        audio: File,
    },
    Video {
        video: File,
    },
}

#[derive(Debug, Deserialize)]
pub struct MessageEntity {
    pub length: usize,
    pub offset: usize,
    #[serde(flatten)]
    pub kind: MessageEntityKind,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageEntityKind {
    BotCommand,
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct File {
    #[serde(rename = "file_id")]
    pub id: Box<str>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: u64,
    pub is_bot: bool,
    pub first_name: Box<str>,
    pub last_name: Option<Box<str>>,
    pub username: Option<Box<str>>,
    pub language_code: Option<Box<str>>,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
    #[serde(flatten)]
    pub kind: ChatKind,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ChatKind {
    Public {
        title: Option<Box<str>>,
        #[serde(flatten)]
        kind: PublicChatKind,
    },
    Private { username: Option<Box<str>> },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PublicChatKind {
    Channel,
    Group,
    SuperGroup,
}

#[derive(Debug, Default, Serialize)]
pub struct SendMessage<'text> {
    pub chat_id: i64,
    pub text: &'text str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disable_web_page_preview: bool, 
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i32>,
}

#[derive(Debug, Default, Serialize)]
pub struct SendAudio<'audio, 'caption> {
    pub chat_id: i64,
    pub audio: &'audio str,
    pub caption: &'caption str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i32>,
}

#[derive(Debug, Default, Serialize)]
pub struct SendVideo<'video, 'caption> {
    pub chat_id: i64,
    pub video: &'video str,
    pub caption: &'caption str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct EditMessageText<'text> {
    pub chat_id: i64,
    pub message_id: i32,
    pub text: &'text str,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessage {
    pub chat_id: i64,
    pub message_id: i32,
}

#[derive(Debug, Serialize)]
pub struct GetMe;

#[derive(Debug, Serialize)]
pub struct DeleteWebhook;

macro_rules! impl_request {
    ($($req:ident $(<$($arg:tt),+>)? => $resp:ident)+) => {
        $(
            impl Request for $req $(<$($arg),+>)? {
                const NAME: &'static str = stringify!($req);
                const URL: &'static str = concat!(
                    env!("TELEGRAM_API_URL"),
                    "/bot",
                    env!("BOT_TOKEN"),
                    "/",
                    stringify!($req),
                );
                type Response = $resp;
            }
        )+
    };
}

impl_request! {
    SetWebhook<'_, '_> => bool
    DeleteWebhook => bool
    SetMyCommands<'_, '_> => bool
    SendMessage<'_> => Message
    GetMe => User
    SendAudio<'_, '_> => Message
    SendVideo<'_, '_> => Message
    EditMessageText<'_> => Message
    DeleteMessage => bool
}

#[derive(Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    #[serde(default)]
    description: String,
    result: Option<T>,
}

const fn e<T>() -> Result<T, std::fmt::Error> { Err(std::fmt::Error) }

struct StringExtractor;

impl Serializer for StringExtractor {
    type Ok = String;
    type Error = std::fmt::Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_str(self,  v: &str) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_u8(self,     v: u8) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_i8(self,     v: i8) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_i16(self,   v: i16) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_i32(self,   v: i32) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_i64(self,   v: i64) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_u16(self,   v: u16) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_u32(self,   v: u32) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_u64(self,   v: u64) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_f32(self,   v: f32) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_f64(self,   v: f64) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> { Ok(v.to_string()) }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<Self::Ok, Self::Error> {
        v.serialize(Self)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_unit_struct(self, _: &str) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> { e() }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> { e() }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> { e() }
    fn serialize_struct(self, _: &str, _: usize) -> Result<Self::SerializeStruct, Self::Error> { e() }
    fn serialize_unit_variant(self, _: &str, _: u32, _: &str) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_tuple_struct(self, _: &str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> { e() }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &str, _: &T) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_tuple_variant(self, _: &str, _: u32, _: &str, _: usize) -> Result<Self::SerializeTupleVariant, Self::Error> { e() }
    fn serialize_struct_variant(self, _: &str, _: u32, _: &str, _: usize) -> Result<Self::SerializeStructVariant, Self::Error> { e() }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &str, _: u32, _: &str, _: &T) -> Result<Self::Ok, Self::Error> { e() }
}

struct FormFiller(Option<Form>);

impl SerializeStruct for FormFiller {
    type Ok = Form;
    type Error = std::fmt::Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize
    {
        self.0 = self.0.take().ok_or(std::fmt::Error)?
            .text(key, value.serialize(StringExtractor)?)
            .into();
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.0.ok_or(std::fmt::Error)
    }
}

impl Serializer for FormFiller {
    type Ok = Form;
    type Error = std::fmt::Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_struct(self, _: &str, _: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_u8(self, _: u8) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_i8(self, _: i8) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_i16(self, _: i16) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_i32(self, _: i32) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_i64(self, _: i64) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_u16(self, _: u16) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_u32(self, _: u32) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_u64(self, _: u64) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_f32(self, _: f32) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_f64(self, _: f64) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_str(self, _: &str) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_bool(self, _: bool) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_char(self, _: char) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_unit_struct(self, _: &str) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> { e() }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> { e() }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> { e() }
    fn serialize_some<T: ?Sized + Serialize>(self,  _: &T) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_unit_variant(self, _: &str, _: u32, _: &str) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_tuple_struct(self, _: &str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> { e() }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &str, _: &T) -> Result<Self::Ok, Self::Error> { e() }
    fn serialize_tuple_variant(self, _: &str, _: u32, _: &str, _: usize) -> Result<Self::SerializeTupleVariant, Self::Error> { e() }
    fn serialize_struct_variant(self, _: &str, _: u32, _: &str, _: usize) -> Result<Self::SerializeStructVariant, Self::Error> { e() }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &str, _: u32, _: &str, _: &T) -> Result<Self::Ok, Self::Error> { e() }
}

fn serialise_into_form<R: Request>(req: &R) -> Result<Form> {
    req.serialize(FormFiller(Some(Form::new())))
        .map_err(|_| format!("{} can't be represented as multipart/form-data", R::NAME).into())
}

#[derive(Default)]
pub struct Client {
    inner: reqwest::Client,
}

impl Client {
    pub fn request<R: Request + Debug>(&self, req: &R) -> impl Future<Output = Result<R::Response>> + Send {
        log::info!("About to send to Telegram: {req:#?}");
        let req = self.inner.get(R::URL).json(req);
        async move {
            match req.send().await?.json().await? {
                TelegramResponse { ok: true, result: Some(result), .. } => Ok(result),
                TelegramResponse { mut description, .. } => Err({
                    description.insert_str(0, ": Telegram API error: ");
                    description.insert_str(0, R::NAME);
                    description.into()
                }),
            }
        }
    }

    /// The `payload` will be referred to in the request as "payload"
    pub fn multipart_request<R: Request + Debug>(&self, req: &R, payload: Part)
        -> impl Future<Output = Result<R::Response>> + '_
    {
        log::info!("About to send to Telegram: {req:#?}");
        let req = serialise_into_form(req)
            .map(|form| self.inner.get(R::URL).multipart(form.part("payload", payload)));

        async move {
            match req?.send().await?.json().await? {
                TelegramResponse { ok: true, result: Some(result), .. } => Ok(result),
                TelegramResponse { mut description, .. } => Err({
                    description.insert_str(0, ": Telegram API error: ");
                    description.insert_str(0, R::NAME);
                    description.into()
                }),
            }
        }
    }
}
