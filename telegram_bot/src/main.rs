use once_cell::sync::Lazy;
use serde_json::json;
use std::{collections::HashMap, env, io, time};
use teloxide::payloads::SendMessageSetters;

use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, MessageEntityKind, MessageId, ParseMode, ChatAction,
};
use teloxide::{prelude::*, utils::command::BotCommands};
use tokio::sync::Mutex;

static BOT_USERNAME: &str = "naive_bing_bot";

static API_HOST: Lazy<String> = Lazy::new(|| env::var("API_HOST").unwrap());
static CHATID_COOKIE: Lazy<Mutex<HashMap<ChatId, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static MSGID_LASTRESP: Lazy<Mutex<HashMap<MessageId, serde_json::Value>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn msg_mentioned(msg: &Message, username: &str) -> bool {
    match msg.parse_entities() {
        Some(entities) => {
            for entity in entities.iter() {
                if *entity.kind() == MessageEntityKind::Mention
                    && entity.text() == format!("@{username}")
                {
                    return true;
                }
            }
            false
        }
        None => false,
    }
}

fn msg_reply_to_username(msg: &Message) -> &str {
    match msg.reply_to_message() {
        Some(replied) => match replied.from() {
            Some(replied_from) => match replied_from.username.as_ref() {
                Some(username) => username.as_str(),
                None => "",
            },
            None => "",
        },
        None => "",
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();
    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                // Filter commands: the next handlers will receive a parsed `SimpleCommand`.
                .filter_command::<Command>()
                // If a command parsing fails, this handler will not be executed.
                .endpoint(handle_cmd),
        )
        .branch(
            // Filtering allow you to filter updates by some condition.
            dptree::filter(|msg: Message| {
                msg.chat.is_private()
                    || msg_mentioned(&msg, BOT_USERNAME)
                    || msg_reply_to_username(&msg) == BOT_USERNAME
            })
            // An endpoint is the last update handler.
            .endpoint(handle_msg_on_prog),
        );

    Dispatcher::builder(bot, handler)
        // Here you specify initial dependencies that all handlers will receive; they can be
        // database connections, configurations, and other auxiliary arguments. It is similar to
        // `actix_web::Extensions`.
        // .dependencies(dptree::deps![parameters])
        // If no handler succeeded to handle an update, this closure will be called.
        .default_handler(|upd| async move {
            log::warn!("Unhandled update: {:?}", upd);
        })
        // If the dispatcher fails for some reason, execute this handler.
        .error_handler(LoggingErrorHandler::with_custom_text(
            "An error has occurred in the dispatcher",
        ))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    log::info!("Shuting down command bot...");
}

async fn handle_msg(bot: Bot, msg: Message) -> ResponseResult<()> {
    let msg_str = msg
        .text()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "msg.text is empty"))?
        .replace(("@".to_string() + BOT_USERNAME).as_str(), "")
        .trim()
        .to_string();
    let id2cookie = CHATID_COOKIE.lock().await;
    let cookie = id2cookie.get(&msg.chat.id);
    if cookie.is_none() {
        bot.send_message(msg.chat.id, "Please set a cookie first.".to_string())
            .await?;
        return Ok(());
    }
    let cookie = cookie.unwrap();
    log::info!(
        "chatid: {} , prompt: {} , cookie: {}",
        msg.chat.id,
        msg_str,
        cookie
    );
    let last_resp = match msg.reply_to_message() {
        Some(replied_msg) => {
            log::info!("reply to id (continue with): {}", replied_msg.id);
            let mut msgid2lastresp = MSGID_LASTRESP.lock().await;
            msgid2lastresp.remove(&replied_msg.id).unwrap_or_else(|| json!({}))
        }
        None => {
            log::info!("no reply; start a new conversation");
            json!({})
        }
    };

    // send HTTP POST to http://localhost:3000/newbing/convo with JSON body:
    // { "prompt": "hello new bing", "cookie": "xxx", "last_resp": {...} }
    let resp = reqwest::Client::new()
        .post(format!("http://{}:3000/newbing/convo", *API_HOST))
        .json(&json!({
            "prompt": msg_str,
            "cookie": cookie,
            "last_resp": last_resp,
        }))
        .timeout(time::Duration::from_secs(30))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // send resp to user
    let resp = &resp["resp"];
    let mut ans = resp["text"]
        .as_str()
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::Other,
            format!("resp has no String typed field \"text\": {resp}"),
        ))?
        .to_owned();
    let attrs = resp["detail"]["sourceAttributions"]
        .as_array()
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::Other,
            "resp[\"detail\"][\"sourceAttributions\"] not found".to_string(),
        ))?;
    if !attrs.is_empty() {
        ans.push_str("\n\n");
    }
    attrs.iter().enumerate().for_each(|(i, x)| {
        let url = x["seeMoreUrl"].as_str().unwrap();
        let name = x["providerDisplayName"].as_str().unwrap();
        let index = i + 1;
        ans.push_str(&format!("{index}: [{name}]({url})\n"));
    });
    #[allow(deprecated)]
    let sent_id = bot
        .send_message(msg.chat.id, ans.as_str())
        .reply_to_message_id(msg.id)
        .parse_mode(ParseMode::Markdown)
        .await?
        .id;
    // store resp
    let mut msgid2lastresp = MSGID_LASTRESP.lock().await;
    msgid2lastresp.insert(sent_id, resp.clone());
    Ok(())
}

async fn handle_msg_on_prog(bot: Bot, msg: Message) -> ResponseResult<()> {
    let msg_str = msg
        .text()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "msg.text is empty"))?
        .replace(("@".to_string() + BOT_USERNAME).as_str(), "")
        .trim()
        .to_string();
    let id2cookie = CHATID_COOKIE.lock().await;
    let cookie = id2cookie.get(&msg.chat.id);
    if cookie.is_none() {
        bot.send_message(msg.chat.id, "Please set a cookie first.".to_string())
            .await?;
        return Ok(());
    }
    let cookie = cookie.unwrap();
    log::info!(
        "chatid: {} , prompt: {} , cookie: {}",
        msg.chat.id,
        msg_str,
        cookie
    );
    let mut last_resp = match msg.reply_to_message() {
        Some(replied_msg) => {
            log::info!("reply to id (continue with): {}", replied_msg.id);
            let mut msgid2lastresp = MSGID_LASTRESP.lock().await;
            msgid2lastresp.remove(&replied_msg.id).unwrap_or_else(|| json!({}))
        }
        None => {
            log::info!("no reply; start a new conversation");
            json!({})
        }
    };
    tokio::spawn(bot.send_chat_action(msg.chat.id, ChatAction::Typing).send());

    #[allow(deprecated)]
    let sent_id = bot
        .send_message(msg.chat.id, "_(waiting for response...)_")
        .parse_mode(ParseMode::Markdown)
        .reply_to_message_id(msg.id)
        .await?
        .id;
    loop {
        log::info!("on progress loop...");
        // send HTTP POST to http://localhost:3000/newbing/onprogress with JSON body:
        // { "prompt": "hello new bing", "cookie": "xxx", "last_resp": {...} }
        let resp = reqwest::Client::new()
            .post(format!("http://{}:3000/newbing/onprogress", *API_HOST))
            .json(&json!({
                "prompt": msg_str,
                "cookie": cookie,
                "last_resp": last_resp,
            }))
            .timeout(time::Duration::from_secs(30))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?; // { resp: {...}, cookie: {...} }

        let resp = &resp["resp"];
        let mut ans = resp["text"]
            .as_str()
            .ok_or_else(|| io::Error::new(
                io::ErrorKind::Other,
                format!("resp has no String typed field \"text\": {resp:#?}"),
            ))?
            .to_owned();

        // append attributions
        let attrs = resp["detail"]["sourceAttributions"].as_array();
        if attrs.is_none() {
            log::info!("resp[\"detail\"][\"sourceAttributions\"] not found");
        } else {
            let attrs = attrs.unwrap();
            if !attrs.is_empty() {
                ans.push_str("\n\nLearn more:\n");
                attrs.iter().enumerate().for_each(|(i, x)| {
                    let url = x["seeMoreUrl"].as_str().unwrap();
                    let name = x["providerDisplayName"].as_str().unwrap();
                    let index = i + 1;
                    ans.push_str(&format!("{index}: [{name}]({url})"));
                    if i < attrs.len() - 1 {
                        ans.push('\n');
                    }
                });
            }
        }
        // append suggested responses:
        let sug_resps = resp["detail"]["suggestedResponses"].as_array();
        if sug_resps.is_none() {
            log::info!("resp[\"detail\"][\"suggestedResponses\"] not found");
        } else {
            let sug_resps = sug_resps.unwrap();
            if !sug_resps.is_empty() {
                ans.push_str("\n\n_Suggested responses:_\n");
                sug_resps.iter().enumerate().for_each(|(i, x)| {
                    let sug = x["text"].as_str().unwrap();
                    let index = i + 1;
                    ans.push_str(&format!("_{index}: {sug}_"));
                    if i < sug_resps.len() - 1 {
                        ans.push('\n');
                    }
                });
            }
        }

        last_resp = resp.clone();
        if !ans.is_empty() {
            let done = resp["done"].as_bool().ok_or_else(|| io::Error::new(
                io::ErrorKind::Other,
                "resp has no bool typed field \"done\"",
            ))?;
            let _ = bot
                .edit_message_text(msg.chat.id, sent_id, ans.as_str())
                .await;
            if done {
                #[allow(deprecated)]
                let _ = bot
                    .edit_message_text(msg.chat.id, sent_id, ans.as_str())
                    .parse_mode(ParseMode::Markdown)
                    .await;
                let mut msgid2lastresp = MSGID_LASTRESP.lock().await;
                let last_resp_map = last_resp.as_object_mut().unwrap();
                last_resp_map.remove("id"); // delete id to start a new query next time
                msgid2lastresp.insert(sent_id, json!(last_resp_map));
                log::info!("exit on progress loop...");
                break;
            }
        }
        // sleep 1s
        tokio::time::sleep(time::Duration::from_secs(2)).await;
    }
    Ok(())
}

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "use /help to see usage.")]
    Start,
    #[command(description = "display this text.")]
    Help,
    #[command(description = "set a cookie by `/cookie <your cookie>`")]
    Cookie(String),
    #[command(description = "show a test message.")]
    Test,
}

async fn handle_cmd(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    log::info!("cmd: {:#?} , chatid: {}", cmd, msg.chat.id);
    match cmd {
        Command::Start => {
            bot.send_message(msg.chat.id, "use /help to see usage.")
                .await?;
        }
        Command::Help => {
            let mut help_msg = Command::descriptions().to_string();
            help_msg.push_str(
                "\n\ncookie is the `_U` cookie of [www.bing.com](https://www.bing.com). Do NOT include `_U=`.\n\
                \nIn private chat, the bot responds to messages directly.\n\
                In group, the bot only responds to messages mentioning (at) it.\n\
                In both case, reply to message of the latest response to continue a conversation.\n\
                "
            );
            #[allow(deprecated)]
            bot.send_message(msg.chat.id, help_msg)
                .parse_mode(ParseMode::Markdown)
                .await?;
        }
        Command::Cookie(cookie) => {
            let cookie = cookie.trim().to_string();
            let mut id2cookie = CHATID_COOKIE.lock().await;
            id2cookie.insert(msg.chat.id, cookie.clone());
            #[allow(deprecated)]
            let sent_id = bot
                .send_message(msg.chat.id, format!("Your cookie is set to `{cookie}` ."))
                .reply_to_message_id(msg.id)
                .parse_mode(ParseMode::Markdown)
                .await?
                .id;
            bot.delete_message(msg.chat.id, sent_id).await?;
            bot.delete_message(msg.chat.id, msg.id).await?;
        }
        Command::Test => {
            log::info!("received: {msg:#?}");
            let options = ["abc", "def", "ghi"].map(|x| InlineKeyboardButton::callback(x, x));
            #[allow(deprecated)]
            let sent = bot
                .send_message(
                    msg.chat.id,
                    "test markdown: [Baidu Haha](https://www.baidu.com)",
                )
                .reply_to_message_id(msg.id)
                .reply_markup(InlineKeyboardMarkup::new([options]))
                .parse_mode(ParseMode::Markdown)
                .await?;
            log::info!("sent: {sent:#?}");
        }
    };
    Ok(())
}
