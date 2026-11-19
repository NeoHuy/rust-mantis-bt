use std::net::SocketAddr;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let db_path = std::env::var("MANTIS_DB_PATH").unwrap_or_else(|_| "mantis.db".to_string());
    let db = rust_mantis_bt::db::Db::open(&db_path)?;

    if let Some(msg) = rust_mantis_bt::handle_cli_command(&db, &args)? {
        println!("{}", msg);
        return Ok(());
    }

    // Start background email dispatcher worker if SendGrid is configured
    if let Some(mailer) = rust_mantis_bt::mailer::SendGridMailer::from_env() {
        println!(
            "SendGrid Mailer initialized (from: {} <{}>). Starting background worker (15s interval)...",
            mailer.from_name, mailer.from_email
        );
        let worker_db = db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;
                if let Err(e) = mailer.dispatch_pending_queue(&worker_db).await {
                    eprintln!("[email-worker] Error dispatching email queue: {}", e);
                }
            }
        });
    } else {
        println!("No SENDGRID_API_KEY provided; email background worker will not start.");
    }

    let addr_str = std::env::var("MANTIS_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    println!("Base URL for issue notifications: {}", db.base_url());

    let app = rust_mantis_bt::build_router(db);

    let addr: SocketAddr = addr_str.parse()?;
    println!("Starting MantisBT on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
