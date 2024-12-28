pub mod models;
pub mod schema;

use crate::models::{Friend, NewFriend};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode},
    response::{Json, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::time::Duration;
use tower_http::{classify::ServerErrorsFailureClass, services::ServeDir, trace::TraceLayer};
use tracing::{info, info_span, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

async fn view_friend(
    State(pool): State<deadpool_diesel::sqlite::Pool>,
    Path(id): Path<i32>,
) -> Result<Json<Friend>, (StatusCode, String)> {
    let conn = pool.get().await.unwrap();

    let res = conn
        .interact(move |conn| {
            self::schema::friends::dsl::friends
                .find(id)
                .select(Friend::as_select())
                .first(conn)
                .unwrap()
        })
        .await
        .map_err(internal_error)?;

    Ok(Json(res))
}

async fn view_all_friends(
    State(pool): State<deadpool_diesel::sqlite::Pool>,
) -> Result<Json<Vec<Friend>>, (StatusCode, String)> {
    let conn = pool.get().await.unwrap();
    let res = conn
        .interact(|conn| {
            self::schema::friends::table
                .select(Friend::as_select())
                .load(conn)
        })
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;

    Ok(Json(res))
}
#[derive(serde::Deserialize)]
struct CreateFriend {
    name: String,
    email: String,
}

async fn create_friend(
    State(pool): State<deadpool_diesel::sqlite::Pool>,
    new_friend_json: Json<NewFriend>,
    new_friend_form: Form<CreateFriend>,
) -> Result<Redirect, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;

    let res;
    if let Some(new_friend_json) = new_friend_json {
        res = add_friend_to_db(conn, new_friend_json).await
    } else if let Some(new_friend_form) = new_friend_form {
        let name = new_friend_form.name;
        let email = new_friend_form.email;

        NewFriend { name, email };
    }

    Ok(Redirect::to(format!("/friends/{}", res.id()).as_str()))
}
async fn add_friend_to_db(conn: _, json: Json<NewFriend>) -> _ {
    conn.interact(|conn| {
        diesel::insert_into(schema::friends::table)
            .values(json)
            .returning(Friend::as_returning())
            .get_result(conn)
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let _ = dotenvy::dotenv();

    let db_url = std::env::var("DATABASE_URL").unwrap();
    let manager = deadpool_diesel::sqlite::Manager::new(db_url, deadpool_diesel::Runtime::Tokio1);
    let pool = deadpool_diesel::sqlite::Pool::builder(manager)
        .build()
        .unwrap();
    // run the migrations on server startup
    {
        let conn = pool.get().await.unwrap();

        info!("Running pending migrations");
        conn.interact(|conn| conn.run_pending_migrations(MIGRATIONS).map(|_| ()))
            .await
            .unwrap()
            .unwrap();
    }

    let addr = "0.0.0.0:3030";
    let app = Router::new()
        .route("/friends/:id", get(view_friend))
        .route("/friends/all", get(view_all_friends))
        .route("/friends/new", post(create_friend))
        .layer({ // LOGGING
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let uri = request.uri().to_string();

                info_span!("http_request", method = ?request.method(), uri, other_field = tracing::field::Empty)
            })
                    .on_request(|_request: &Request<_>, _span: &Span| {
                        // You can use `_span.record("some_other_field", value)` in one of these
                        // closures to attach a value to the initially empty field in the info_span
                        // created above.
                        info!("");
                    })
                    .on_response(|_response: &Response, _latency: Duration, _span: &Span| {
                    })
                    .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {
                    })
                    .on_eos(
                        |_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span| {
                        },
                    )
                    .on_failure(
                        |_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
                            info!("error");
                        },
                    )
        })
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(pool);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::debug!("Listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
