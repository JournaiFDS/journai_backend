pub mod routes;

use std::{borrow::BorrowMut, sync::Arc};

use axum::{routing::post, Extension, Router};
use color_eyre::eyre::Ok;
use mongodb::{options::ClientOptions, Client};
use routes::{new_journal_entry, JournalEntry};
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    dotenvy::dotenv()?;

    // Gestion des erreurs
    color_eyre::install()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env());

    // Base de donnée
    let mut options = ClientOptions::parse(std::env::var("MONGO")?).await?;
    options.app_name = Some("Journai".to_string());
    let mongo_client = Arc::new(Client::with_options(options)?);
    let database: Arc<mongodb::Database> = Arc::new(mongo_client.database("journai"));
    let entries_collection = Arc::new(database.collection::<JournalEntry>("entries"));

    let app = Router::new().route("/", post(new_journal_entry));

    let listener = TcpListener::bind(format!(
        "{}:{}",
        option_env!("SERVER_IP").unwrap_or("0.0.0.0"),
        option_env!("SERVER_PORT").unwrap_or("3000")
    ))
    .await?;

    axum::serve(listener, app).await?;
    Ok(())
}
