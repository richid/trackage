use crate::db::{Database, SqliteDatabase};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
    routing::get,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tracing::{error, info};

const INDEX_HTML: &str = include_str!("../static/index.html");

type Db = Arc<Mutex<SqliteDatabase>>;

async fn index() -> Response {
    ([(header::CONTENT_TYPE, "text/html")], INDEX_HTML).into_response()
}

async fn api_packages(State(db): State<Db>) -> Response {
    let db = db.lock().unwrap();
    match db.get_all_packages_with_status() {
        Ok(packages) => Json(packages).into_response(),
        Err(err) => {
            error!(error = %err, "Failed to query packages");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn start(db_path: String, port: u16, running: Arc<AtomicBool>) {
    let db = match SqliteDatabase::open(&db_path) {
        Ok(db) => Arc::new(Mutex::new(db)),
        Err(err) => {
            error!(error = %err, "Web server failed to open database");
            return;
        }
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/packages", get(api_packages))
        .with_state(db);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime for web server");

    rt.block_on(async {
        let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
            Ok(l) => l,
            Err(err) => {
                error!(error = %err, port, "Web server failed to bind");
                return;
            }
        };

        info!(port, "Web server listening");

        let shutdown = async move {
            while running.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            info!("Web server shutting down");
        };

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
            .expect("Web server error");
    });
}
