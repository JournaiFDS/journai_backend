pub mod routes;

use std::sync::Arc;

use axum::{http::Method, routing::get, Extension, Router};
use color_eyre::eyre::Ok;
use mongodb::{
    bson::doc,
    options::{ClientOptions, IndexOptions},
    Client, IndexModel,
};
use routes::{create_journal_entry, delete_journal_entry, list_journal_entries, JournalEntry};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
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
    let database = Arc::new(mongo_client.database("journai"));
    let entries_collection = Arc::new(database.collection::<JournalEntry>("entries"));
    entries_collection
        .create_index(
            IndexModel::builder()
                .keys(doc! { "date": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
            None,
        )
        .await?;

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route(
            "/",
            get(list_journal_entries)
                .post(create_journal_entry)
                .delete(delete_journal_entry),
        )
        .layer(Extension(entries_collection))
        .layer(cors);

    let listener = TcpListener::bind(format!(
        "{}:{}",
        option_env!("SERVER_IP").unwrap_or("0.0.0.0"),
        option_env!("SERVER_PORT").unwrap_or("3000")
    ))
    .await?;

    axum::serve(listener, app).await?;
    Ok(())
}
