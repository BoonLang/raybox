use anyhow::Result;
use axum::{routing::get_service, Router};
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::ServeDir;

pub fn run(directory: &str, port: u16) -> Result<()> {
    log::info!("Serving {} on port {}", directory, port);

    // Convert to absolute path
    let serve_path = PathBuf::from(directory);
    let serve_path = if serve_path.is_absolute() {
        serve_path
    } else {
        std::env::current_dir()?.join(&serve_path)
    };

    if !serve_path.exists() {
        anyhow::bail!("Directory does not exist: {}", serve_path.display());
    }

    if !serve_path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", serve_path.display());
    }

    println!("✓ Serving directory: {}", serve_path.display());
    println!("  Port: {}", port);
    println!("  URL: http://localhost:{}", port);
    println!();
    println!("  Press Ctrl+C to stop");
    println!();

    // Use tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { serve_directory(serve_path, port).await })?;

    Ok(())
}

async fn serve_directory(path: PathBuf, port: u16) -> Result<()> {
    // Create service for serving files
    let serve_dir = ServeDir::new(path).append_index_html_on_directories(true);

    // Create router
    let app = Router::new().nest_service("/", get_service(serve_dir));

    // Bind to address
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    log::info!("Starting server on {}", addr);

    // Run server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
