use std::{fmt::Debug, sync::Arc, vec};

use async_openai::{
    error::OpenAIError,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use axum::{extract, http::StatusCode, Extension, Json};
use axum_thiserror::ErrorStatus;
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Europe::Paris;
use futures_util::TryStreamExt;
use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JournalEntry {
    pub date: NaiveDate,
    pub rate: f32,
    pub short_summary: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateJournalEntry {
    pub name: String,
    pub summary: String,
    pub date: NaiveDate,
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
    #[error(transparent)]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Mongo(mongodb::error::Error),
}

pub async fn create_journal_entry(
    Extension(mongo_entries): Extension<Arc<Collection<JournalEntry>>>,
    journal_entry: extract::Json<CreateJournalEntry>,
) -> Result<Json<JournalEntry>, CreateJournalEntryError> {
    let entry_message = ChatCompletionRequestMessage::System(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(include_str!("./journal_entry_message.txt"))
            .build()
            .map_err(CreateJournalEntryError::OpenAI)?,
    );
    let user_message = ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessageArgs::default()
            .content(format!(
                "{} ({}): {}",
                journal_entry.name, journal_entry.date, journal_entry.summary
            ))
            .build()
            .map_err(CreateJournalEntryError::OpenAI)?,
    );
    let completion_request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .messages(vec![entry_message, user_message])
        .n(1)
        .build()
        .map_err(CreateJournalEntryError::OpenAI)?;

    let response = Client::new()
        .chat()
        .create(completion_request)
        .await
        .map_err(CreateJournalEntryError::OpenAI)?;

    let json = response
        .choices
        .get(0)
        .map(|o| o.message.clone().content)
        .flatten()
        .ok_or(CreateJournalEntryError::NoOutput)?;

    let json = serde_json::from_str::<JournalEntry>(&json)
        .map_err(CreateJournalEntryError::Serialization)?;

    if let Err(_) = mongo_entries.insert_one(json.clone(), None).await {
        let date = Paris
            .from_local_datetime(&json.date.and_hms_opt(0, 0, 0).unwrap())
            .unwrap();

        mongo_entries
            .update_one(
                doc! { "date": date },
                doc! {
                    "$set": {
                        "short_summary": json.short_summary.clone(),
                        "rate": json.rate.clone()
                    }
                },
                None,
            )
            .await
            .map_err(CreateJournalEntryError::Mongo)?;
    }

    Ok(Json(json))
}

#[derive(Error, Debug, ErrorStatus)]
pub enum ListJournalEntryError {
    #[error(transparent)]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Mongo(mongodb::error::Error),
}

pub async fn list_journal_entries(
    Extension(mongo_entries): Extension<Arc<Collection<JournalEntry>>>,
) -> Result<Json<Vec<JournalEntry>>, ListJournalEntryError> {
    Ok(Json(
        mongo_entries
            .find(None, None)
            .await
            .map_err(ListJournalEntryError::Mongo)?
            .try_collect()
            .await
            .map_err(ListJournalEntryError::Mongo)?,
    ))
}

#[derive(Error, Debug, ErrorStatus)]
pub enum DeleteJournalEntryError {
    #[error(transparent)]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Bson(bson::ser::Error),
    #[error(transparent)]
    #[status(StatusCode::INTERNAL_SERVER_ERROR)]
    Mongo(mongodb::error::Error),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DeleteJournalEntry {
    pub date: NaiveDate,
}

pub async fn delete_journal_entry(
    Extension(mongo_entries): Extension<Arc<Collection<JournalEntry>>>,
    payload: Json<DeleteJournalEntry>,
) -> Result<(), DeleteJournalEntryError> {
    let _ = mongo_entries
        .delete_one(
            bson::to_document(&payload.0).map_err(DeleteJournalEntryError::Bson)?,
            None,
        )
        .await
        .map_err(DeleteJournalEntryError::Mongo)?;

    Ok(())
}
