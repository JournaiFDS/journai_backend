use std::fmt::Debug;

use async_openai::{
    error::OpenAIError,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use axum::{extract, http::StatusCode, Json};
use axum_thiserror::ErrorStatus;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Error, Debug, ErrorStatus)]
pub enum CreateJournalEntryError {
    #[error(transparent)]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    OpenAI(OpenAIError),
    #[error("no output from GPT-4")]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    NoOutput,
    #[error(transparent)]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Serialization(serde_json::Error),
}

pub async fn new_journal_entry(
    journal_entry: extract::Json<CreateJournalEntry>,
) -> Result<Json<JournalEntry>, CreateJournalEntryError> {
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
                            .map_err(CreateJournalEntryError::OpenAI)?,
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
                            .map_err(CreateJournalEntryError::OpenAI)?,
                    ),
                ])
                .n(1)
                .build()
                .map_err(CreateJournalEntryError::OpenAI)?,
        )
        .await
        .map_err(CreateJournalEntryError::OpenAI)?;

    let json = r
        .choices
        .get(0)
        .map(|o| o.message.clone().content)
        .flatten()
        .ok_or(CreateJournalEntryError::NoOutput)?;

    let json = serde_json::from_str::<JournalEntry>(&json)
        .map_err(CreateJournalEntryError::Serialization)?;

    Ok(Json(json))
}
