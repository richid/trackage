use crate::db::{Database, NewPackage, SqliteDatabase};
use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tracking_numbers::track;
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

#[derive(Deserialize)]
struct ValidateRequest {
    tracking_number: String,
}

#[derive(Serialize)]
struct TrackingMatch {
    tracking_number: String,
    courier: String,
    service: String,
    tracking_url: String,
}

async fn api_validate(Json(req): Json<ValidateRequest>) -> Json<Vec<TrackingMatch>> {
    let cleaned: String = req
        .tracking_number
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    let matches = match track(&cleaned) {
        Some(result) => vec![TrackingMatch {
            tracking_number: result.tracking_number,
            courier: result.courier,
            service: result.service,
            tracking_url: result.tracking_url,
        }],
        None => vec![],
    };

    Json(matches)
}

#[derive(Deserialize)]
struct AddPackageRequest {
    tracking_number: String,
    courier: String,
    service: String,
    tracking_url: String,
}

async fn api_add_package(State(db): State<Db>, Json(req): Json<AddPackageRequest>) -> Response {
    let new_package = NewPackage {
        tracking_number: req.tracking_number,
        courier: req.courier,
        service: req.service,
        tracking_url: req.tracking_url,
        source_email_uid: 0,
        source_email_subject: None,
        source_email_from: None,
        source_email_date: Utc::now(),
    };

    let mut db = db.lock().unwrap();
    match db.insert_package(&new_package) {
        Ok(true) => StatusCode::CREATED.into_response(),
        Ok(false) => StatusCode::CONFLICT.into_response(),
        Err(err) => {
            error!(error = %err, "Failed to insert package");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn api_delete_package(State(db): State<Db>, Path(id): Path<i64>) -> Response {
    let mut db = db.lock().unwrap();
    match db.delete_package(id) {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            error!(error = %err, package_id = id, "Failed to delete package");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn api_package_history(State(db): State<Db>, Path(id): Path<i64>) -> Response {
    let db = db.lock().unwrap();
    match db.get_package_status_history(id) {
        Ok(entries) => Json(entries).into_response(),
        Err(err) => {
            error!(error = %err, package_id = id, "Failed to query package history");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn api_package_rescan(State(db): State<Db>, Path(id): Path<i64>) -> Response {
    let mut db = db.lock().unwrap();
    match db.delete_all_package_status(id) {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(err) => {
            error!(error = %err, package_id = id, "Failed to delete all package history");
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
        .route("/api/packages", get(api_packages).post(api_add_package))
        .route("/api/packages/validate", post(api_validate))
        .route("/api/packages/{id}", delete(api_delete_package))
        .route("/api/packages/{id}/history", get(api_package_history))
        .route("/api/packages/{id}/rescan", post(api_package_rescan))
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
