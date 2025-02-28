mod config;
mod foxglove_server;
mod gamepad;
mod messages;
mod tailscale;

use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{self, AsyncBufReadExt},
    process::Command,
};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use foxglove_server::create_foxglove_url;
use gamepad::start_gamepad_reader;
use tailscale::TailscaleStatus;

use schemars::schema_for;
use tracing::*;

use crate::messages::InputMessage;

const ZENOH_TCP_DISCOVERY_PORT: u16 = 7436;

const HAMILTON_FOXGLOVE_LAYOUT_ID: &str = "0948be25-5808-40db-a1d3-75e7810fe349";
const HOPPER_FOXGLOVE_LAYOUT_ID: &str = "ea22e72c-f654-4743-925a-7143a510d390";
const FLATPAK_CHROME_PATH: &str =
    "/var/lib/flatpak/app/com.google.Chrome/x86_64/stable/active/export/bin/com.google.Chrome";

#[derive(Parser)]
#[command(author, version)]
struct Args {
    #[clap(short, long, default_value = "hamilton")]
    mode: Mode,

    /// The key expression to publish onto.
    #[clap(short, long, default_value = "remote-control/gamepad")]
    gamepad_topic: String,

    /// A configuration file.
    #[clap(long)]
    zenoh_config: Option<String>,

    /// Loop sleep time
    #[clap(short, long, default_value = "50")]
    sleep_ms: u64,

    /// verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// foxglove bind address
    #[clap(long, default_value = "127.0.0.1:8765")]
    host: SocketAddr,

    #[clap(long, default_value = "david-weis")]
    foxglove_user: String,

    #[clap(long)]
    foxglove_layout_id: Option<String>,

    /// Open browser
    #[clap(short, long, default_value = "true")]
    browser: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Hamilton,
    Guppy,
    Hopper,
}

#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let args: Args = Args::parse();
    setup_tracing(args.verbose);

    let schema = schema_for!(InputMessage);
    info!(
        "Message schema:\n{}",
        serde_json::to_string_pretty(&schema)?
    );

    start_gamepad_reader(args.sleep_ms).await?;

    let layout_id = match args.mode {
        Mode::Hamilton => HAMILTON_FOXGLOVE_LAYOUT_ID,
        Mode::Guppy => HAMILTON_FOXGLOVE_LAYOUT_ID,
        Mode::Hopper => HOPPER_FOXGLOVE_LAYOUT_ID,
    };

    let foxglove_link = create_foxglove_url(
        &args.foxglove_user,
        &args.host.ip().to_string(),
        &args.host.port().to_string(),
        layout_id,
    );

    info!("Foxglove link {foxglove_link}");

    if args.browser {
        // open::that(foxglove_link)?;
        // open::with(&foxglove_link, "chrome")?;
        let mut browser_process_handle = Command::new(FLATPAK_CHROME_PATH)
            .arg("--start-fullscreen")
            .arg(foxglove_link)
            .arg("--noerrdialogs")
            .arg("--no-first-run")
            .arg("--start-maximized")
            .spawn()?;

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = read_line() => {}
            _ = browser_process_handle.wait() => {
                info!("Browser process exited");
            }
        };
    } else {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = read_line() => {}
        };
    }

    Ok(())
}

async fn read_line() -> anyhow::Result<()> {
    let mut stdin = io::BufReader::new(io::stdin());
    stdin.read_line(&mut String::new()).await?;
    Ok(())
}

pub fn setup_tracing(verbosity_level: u8) {
    let filter = match verbosity_level {
        0 => tracing::level_filters::LevelFilter::INFO,
        1 => tracing::level_filters::LevelFilter::DEBUG,
        2 => tracing::level_filters::LevelFilter::TRACE,
        _ => tracing::level_filters::LevelFilter::TRACE,
    };
    tracing_subscriber::fmt().with_max_level(filter).init();
}
