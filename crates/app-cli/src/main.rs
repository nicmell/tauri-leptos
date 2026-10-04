//! The app as a standalone server: its pages, server functions and site
//! files on one address. `cargo leptos watch` runs it without arguments.

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};

use clap::Parser;
use leptos::prelude::get_configuration;
use tauri_leptos_ui::config::AppConfig;
use tower_http::services::ServeDir;

#[derive(Parser)]
#[command(name = "tauri-leptos-cli", version, about = "tauri-leptos server")]
struct Cli {
    /// Bind address
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    host: IpAddr,
    /// Bind port
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();
    let options = get_configuration(None)?.leptos_options;
    let site_root = options.site_root.to_string();
    let app = tauri_leptos_ui::server::router(options, AppConfig::default())
        .fallback_service(ServeDir::new(site_root));

    let listener = tokio::net::TcpListener::bind((cli.host, cli.port)).await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use clap::{CommandFactory, Parser};

    use super::Cli;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn defaults_are_the_cargo_leptos_site_addr() {
        let workspace: toml::Table = include_str!("../../../Cargo.toml")
            .parse()
            .expect("parse the workspace manifest");
        let site_addr = workspace["workspace"]["metadata"]["leptos"][0]["site-addr"]
            .as_str()
            .expect("site-addr");
        let cli = Cli::parse_from(["tauri-leptos-cli"]);
        assert_eq!(SocketAddr::new(cli.host, cli.port).to_string(), site_addr);
    }
}
