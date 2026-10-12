use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = std::env::var("MANTIS_DB_PATH").unwrap_or_else(|_| "mantis.db".to_string());
    let addr_str = std::env::var("MANTIS_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let db = rust_mantis_bt::db::Db::open(&db_path)?;
    let app = rust_mantis_bt::build_router(db);

    let addr: SocketAddr = addr_str.parse()?;
    println!("Starting MantisBT on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
