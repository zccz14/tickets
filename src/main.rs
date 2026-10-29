use std::{net::SocketAddr, path::PathBuf};

use tickets::{Options, bootstrap, router};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

fn parse_args() -> Options {
    let mut options = Options::new(PathBuf::from("./data"));
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => {
                if let Some(value) = args.next() {
                    options.listen = value.parse().expect("--listen 需要 host:port");
                }
            }
            "--port" => {
                if let Some(value) = args.next() {
                    let port: u16 = value.parse().expect("--port 需要数字");
                    options.listen = SocketAddr::from(([0, 0, 0, 0], port));
                }
            }
            "--data-dir" => {
                if let Some(value) = args.next() {
                    options.data_dir = PathBuf::from(value);
                }
            }
            "--public-base-url" => {
                options.public_base_url = args.next();
            }
            "--dev-user" => {
                options.dev_user = args.next();
            }
            "--help" | "-h" => {
                println!(
                    "tickets [--listen host:port] [--port n] [--data-dir path] [--public-base-url url] [--dev-user <auth-mini-subject>]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("未知参数: {other}");
                std::process::exit(2);
            }
        }
    }
    options
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let options = parse_args();
    let listen = options.listen;
    let data_dir = options.data_dir.clone();
    let state = bootstrap(options).await?;
    tracing::info!(
        "Tickets 启动于 http://{listen}，数据目录 {}",
        data_dir.display()
    );

    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
