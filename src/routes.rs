use std::{fmt::Debug, sync::Arc};

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, CreateCompletionRequest, CreateCompletionRequestArgs,
        Role,
    },
    Client,
};
use axum::{extract, http::StatusCode, Extension, Json};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct JournalEntry {
    pub date: NaiveDate,
    pub rate: f32,
    pub short_summary: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateJournalEntry {
    pub name: String,
    pub summary: String,
}

pub async fn new_journal_entry(
    journal_entry: extract::Json<CreateJournalEntry>,
) -> Result<Json<JournalEntry>, (StatusCode, String)> {
    let r = Client::new()
        .chat()
        .create(
            CreateChatCompletionRequestArgs::default()
                .model("gpt-4")
                .messages(vec![
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(include_str!("./journal_entry_message.txt"))
                            .build()
                            .unwrap(),
                    ),
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(format!(
                                "{} ({}): {}",
                                journal_entry.name,
                                Utc::now().date_naive(),
                                journal_entry.summary
                            ))
                            .build()
                            .unwrap(),
                    ),
                ])
                .n(1)
                .build()
                .map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to build OpenAI prompt.".to_string(),
                    )
                })?,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", e)))?;

    let json = match r
        .choices
        .get(0)
        .map(|o| o.message.clone().content)
        .flatten()
    {
        Some(message) => Ok(message),
        None => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "No output from GPT.".to_string(),
        )),
    };

    match serde_json::from_str::<JournalEntry>(&json?) {
        Ok(entry) => Ok(Json(entry)),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to serialize GPT's output.".to_string(),
        )),
    }
}
