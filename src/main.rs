mod config;
mod error;
mod gamepad;
mod messages;
mod tailscale;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use error::ErrorWrapper;
use gamepad::{start_gamepad_reader, start_schema_queryable};
use tailscale::TailscaleStatus;

use schemars::schema_for;
use tracing::*;
use zenoh::{config::Config, prelude::r#async::*};

use crate::messages::InputMessage;

const ZENOH_TCP_DISCOVERY_PORT: u16 = 7436;

#[derive(Parser)]
#[command(author, version)]
struct Args {
    #[clap(short, long, default_value = "hamilton")]
    mode: Mode,

    /// The key expression to publish onto.
    #[clap(short, long, default_value = "remote-control/gamepad")]
    gamepad_topic: String,

    /// Endpoints to connect to.
    #[clap(short, long)]
    connect: Vec<zenoh_config::EndPoint>,

    /// Endpoints to listen on.
    #[clap(short, long)]
    listen: Vec<zenoh_config::EndPoint>,

    /// A configuration file.
    #[clap(long)]
    zenoh_config: Option<String>,

    /// Loop sleep time
    #[clap(short, long, default_value = "50")]
    sleep_ms: u64,

    /// verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Hamilton,
}

#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let args: Args = Args::parse();
    setup_tracing(args.verbose);

    let mut zenoh_config = if let Some(conf_file) = &args.zenoh_config {
        Config::from_file(conf_file).map_err(ErrorWrapper::ZenohError)?
    } else {
        Config::default()
    };

    if !args.connect.is_empty() {
        zenoh_config.connect.endpoints.clone_from(&args.connect);
    }
    if !args.listen.is_empty() {
        zenoh_config.listen.endpoints.clone_from(&args.listen);
    }

    let tailscale_status = TailscaleStatus::read_from_command().await?;

    // listening address
    for local_address in &tailscale_status.tailscale_ip_list {
        let address: std::net::IpAddr = local_address.parse().context("Failed to parse address")?;
        if !address.is_ipv4() {
            // skip IPv6 because pain
            continue;
        }
        let tcp = zenoh_config::EndPoint::new("tcp", format!("{}:{}", local_address, 0), "", "")
            .map_err(ErrorWrapper::ZenohError)?;
        zenoh_config.listen.endpoints.push(tcp)
    }

    // peer address
    for peer in tailscale_status.peers.values() {
        if !peer.host_name.to_lowercase().contains("hamilton") {
            continue;
        }
        for local_address in &peer.tailscale_ip_list {
            let address: std::net::IpAddr =
                local_address.parse().context("Failed to parse address")?;
            if !address.is_ipv4() {
                // skip IPv6 because pain
                continue;
            }
            let tcp = zenoh_config::EndPoint::new(
                "tcp",
                format!("{}:{}", local_address, ZENOH_TCP_DISCOVERY_PORT),
                "",
                "",
            )
            .map_err(ErrorWrapper::ZenohError)?;
            zenoh_config.connect.endpoints.push(tcp)
        }
    }

    // log config
    if let Some(config) = &args.zenoh_config {
        info!("Using zenoh config {:?}", config);
    }
    if !zenoh_config.connect.endpoints.is_empty() {
        info!("Connecting to {:?}", zenoh_config.connect.endpoints);
    }
    if !zenoh_config.listen.endpoints.is_empty() {
        info!("Listening on {:?}", zenoh_config.listen.endpoints);
    }

    info!("Publishing on topic {:?}", args.gamepad_topic);
    debug!("Starting zenoh session");
    let zenoh_session = zenoh::open(zenoh_config)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)?
        .into_arc();

    let schema = schema_for!(InputMessage);
    info!(
        "Message schema:\n{}",
        serde_json::to_string_pretty(&schema)?
    );

    start_schema_queryable(zenoh_session.clone(), &args.gamepad_topic).await?;
    let gamepad_future = tokio::spawn(async move {
        if let Err(error) =
            start_gamepad_reader(zenoh_session.clone(), &args.gamepad_topic, args.sleep_ms).await
        {
            error!("Gamepad reader encountered an error {error:?}");
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = gamepad_future => {}
    };
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
