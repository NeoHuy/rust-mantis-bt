use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use rust_mantis_bt::build_router;
use rust_mantis_bt::db::Db;
use rust_mantis_bt::mailer::{SendGridMailer, SendGridPayload};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

async fn spawn_mock_sendgrid(
    status_code: StatusCode,
    recorded_requests: Arc<Mutex<Vec<(HeaderMap, serde_json::Value)>>>,
) -> (String, tokio::sync::oneshot::Sender<()>) {
    let requests = recorded_requests.clone();
    let app = axum::Router::new().route(
        "/v3/mail/send",
        post(
            move |headers: HeaderMap, axum::Json(payload): axum::Json<serde_json::Value>| {
                let requests = requests.clone();
                async move {
                    requests.lock().unwrap().push((headers, payload));
                    status_code.into_response()
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    (format!("http://{}/v3/mail/send", addr), tx)
}

#[test]
fn test_sendgrid_payload_serialization() {
    let payload = SendGridPayload::new(
        "test@example.com",
        "no-reply@example.com",
        "Mantis Bug Tracker",
        "Issue #123 updated",
        "Status changed to resolved.",
    );
    let json_val = serde_json::to_value(&payload).unwrap();
    assert_eq!(
        json_val["personalizations"][0]["to"][0]["email"],
        "test@example.com"
    );
    assert_eq!(json_val["from"]["email"], "no-reply@example.com");
    assert_eq!(json_val["from"]["name"], "Mantis Bug Tracker");
    assert_eq!(json_val["subject"], "Issue #123 updated");
    assert_eq!(json_val["content"][0]["type"], "text/plain");
    assert_eq!(
        json_val["content"][0]["value"],
        "Status changed to resolved."
    );
}

#[tokio::test]
async fn test_sendgrid_send_success() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let (endpoint, _shutdown) = spawn_mock_sendgrid(StatusCode::ACCEPTED, recorded.clone()).await;

    let mailer = SendGridMailer::new(
        "SG.fake_key_123".to_string(),
        "no-reply@example.com".to_string(),
        "Mantis BT".to_string(),
    )
    .with_endpoint(endpoint);

    let result = mailer
        .send(
            "dev@example.com",
            "Bug #1 reported",
            "There is a new bug reported.",
        )
        .await;

    assert!(result.is_ok());

    let reqs = recorded.lock().unwrap();
    assert_eq!(reqs.len(), 1);
    let (headers, payload) = &reqs[0];
    let auth = headers.get("authorization").unwrap().to_str().unwrap();
    assert_eq!(auth, "Bearer SG.fake_key_123");
    assert_eq!(
        payload["personalizations"][0]["to"][0]["email"],
        "dev@example.com"
    );
}

#[tokio::test]
async fn test_sendgrid_dispatch_pending_queue_success() {
    let db = Db::open(":memory:").unwrap();
    {
        let conn = db.conn();
        db.enqueue_email(&conn, "user1@example.com", "Subject 1", "Body 1")
            .unwrap();
        db.enqueue_email(&conn, "user2@example.com", "Subject 2", "Body 2")
            .unwrap();
    }

    assert_eq!(db.list_queued_emails().unwrap().len(), 2);

    let recorded = Arc::new(Mutex::new(Vec::new()));
    let (endpoint, _shutdown) = spawn_mock_sendgrid(StatusCode::ACCEPTED, recorded.clone()).await;

    let mailer = SendGridMailer::new(
        "SG.test_key".to_string(),
        "no-reply@example.com".to_string(),
        "Mantis BT".to_string(),
    )
    .with_endpoint(endpoint);

    let count = mailer.dispatch_pending_queue(&db).await.unwrap();
    assert_eq!(count, 2);

    // Queue in db should be empty now
    assert_eq!(db.list_queued_emails().unwrap().len(), 0);

    // Mock server received 2 calls
    assert_eq!(recorded.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn test_sendgrid_dispatch_queue_failure_preserves_queue() {
    let db = Db::open(":memory:").unwrap();
    {
        let conn = db.conn();
        db.enqueue_email(&conn, "user1@example.com", "Subject 1", "Body 1")
            .unwrap();
    }

    let recorded = Arc::new(Mutex::new(Vec::new()));
    let (endpoint, _shutdown) =
        spawn_mock_sendgrid(StatusCode::INTERNAL_SERVER_ERROR, recorded.clone()).await;

    let mailer = SendGridMailer::new(
        "SG.test_key".to_string(),
        "no-reply@example.com".to_string(),
        "Mantis BT".to_string(),
    )
    .with_endpoint(endpoint);

    let count = mailer.dispatch_pending_queue(&db).await.unwrap();
    assert_eq!(count, 0);

    // Queue in db should STILL have the email!
    assert_eq!(db.list_queued_emails().unwrap().len(), 1);
}

#[tokio::test]
async fn test_http_manage_email_queue_flush_with_sendgrid() {
    let db = Db::open(":memory:").unwrap();
    {
        let conn = db.conn();
        db.enqueue_email(&conn, "flush_user@example.com", "Test Subject", "Test Body")
            .unwrap();
    }

    let recorded = Arc::new(Mutex::new(Vec::new()));
    let (endpoint, _shutdown) = spawn_mock_sendgrid(StatusCode::ACCEPTED, recorded.clone()).await;

    // Set env vars for SendGrid
    std::env::set_var("SENDGRID_API_KEY", "SG.mock_flush_key");
    std::env::set_var("SENDGRID_ENDPOINT", &endpoint);
    std::env::set_var("SENDGRID_FROM_EMAIL", "notify@example.com");
    std::env::set_var("SENDGRID_FROM_NAME", "Mantis Tracker");

    let app = build_router(db.clone());

    // Login as administrator
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=administrator&password=root"))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = res
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // Call POST /manage_email_queue_flush
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/manage_email_queue_flush")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Clean up env vars
    std::env::remove_var("SENDGRID_API_KEY");
    std::env::remove_var("SENDGRID_ENDPOINT");
    std::env::remove_var("SENDGRID_FROM_EMAIL");
    std::env::remove_var("SENDGRID_FROM_NAME");

    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let location = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.contains("dispatched"));
    assert_eq!(recorded.lock().unwrap().len(), 1);
    assert_eq!(db.list_queued_emails().unwrap().len(), 0);
}
