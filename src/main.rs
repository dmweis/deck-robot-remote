mod config;
mod error;
mod foxglove_server;
mod gamepad;
mod messages;
mod tailscale;

use std::{net::SocketAddr, sync::Arc};
use tokio::io::{self, AsyncBufReadExt};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use error::ErrorWrapper;
use foxglove_server::{create_foxglove_url, start_foxglove_bridge, FoxgloveServerConfiguration};
use gamepad::{start_gamepad_reader, start_schema_queryable};
use tailscale::TailscaleStatus;

use schemars::schema_for;
use tracing::*;
use zenoh::{config::Config, prelude::r#async::*};

use once_cell::sync::Lazy;
use prost_reflect::DescriptorPool;

use crate::messages::InputMessage;

const ZENOH_TCP_DISCOVERY_PORT: u16 = 7436;

const HAMILTON_FOXGLOVE_LAYOUT_ID: &str = "0948be25-5808-40db-a1d3-75e7810fe349";
const HOPPER_FOXGLOVE_LAYOUT_ID: &str = "ea22e72c-f654-4743-925a-7143a510d390";

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

    /// foxglove bind address
    #[clap(long, default_value = "127.0.0.1:8765")]
    host: SocketAddr,

    #[clap(long, default_value = "david-weis")]
    foxglove_user: String,

    #[clap(long)]
    foxglove_layout_id: Option<String>,

    /// Open browser
    #[clap(short, long)]
    browser: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Hamilton,
    Hopper,
}

#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let args: Args = Args::parse();
    setup_tracing(args.verbose);

    let zenoh_session = start_zenoh_session(&args).await?;

    info!("Publishing on topic {:?}", args.gamepad_topic);

    let schema = schema_for!(InputMessage);
    info!(
        "Message schema:\n{}",
        serde_json::to_string_pretty(&schema)?
    );

    start_schema_queryable(zenoh_session.clone(), &args.gamepad_topic).await?;
    start_gamepad_reader(zenoh_session.clone(), &args.gamepad_topic, args.sleep_ms).await?;

    // read foxglove config
    let foxglove_config = match args.mode {
        Mode::Hamilton => {
            let config = include_str!("../config/hamilton_config.yaml");
            let config: FoxgloveServerConfiguration = serde_yaml::from_str(config)?;
            config
        }
        Mode::Hopper => {
            let config = include_str!("../config/hopper_config.yaml");
            let config: FoxgloveServerConfiguration = serde_yaml::from_str(config)?;
            config
        }
    };

    start_foxglove_bridge(foxglove_config, args.host, zenoh_session.clone()).await?;

    let layout_id = match args.mode {
        Mode::Hamilton => HAMILTON_FOXGLOVE_LAYOUT_ID,
        Mode::Hopper => HOPPER_FOXGLOVE_LAYOUT_ID,
    };

    let foxglove_link = create_foxglove_url(
        &args.foxglove_user,
        &args.host.ip().to_string(),
        &args.host.port().to_string(),
        layout_id,
    );

    info!("Foxglove link {foxglove_link}");
    open::that(foxglove_link)?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = read_line() => {}
    };
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

static FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/file_descriptor_set.bin"));

static DESCRIPTOR_POOL: Lazy<DescriptorPool> = Lazy::new(|| {
    DescriptorPool::decode(FILE_DESCRIPTOR_SET).expect("Failed to load file descriptor set")
});

/// protobuf
pub mod foxglove {
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/foxglove.rs"));
}

pub mod hopper {
    #![allow(non_snake_case)]
    include!(concat!(env!("OUT_DIR"), "/hopper.rs"));
}

async fn start_zenoh_session(args: &Args) -> anyhow::Result<Arc<Session>> {
    // load config
    let mut zenoh_config = if let Some(conf_file) = &args.zenoh_config {
        Config::from_file(conf_file).map_err(ErrorWrapper::ZenohError)?
    } else {
        Config::default()
    };
    // add arg endpoints
    if !args.connect.is_empty() {
        zenoh_config.connect.endpoints.clone_from(&args.connect);
    }
    if !args.listen.is_empty() {
        zenoh_config.listen.endpoints.clone_from(&args.listen);
    }

    // add tailscale config
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
        // select target based on host
        match args.mode {
            Mode::Hamilton => {
                if !peer.host_name.to_lowercase().contains("hamilton") {
                    // skip others
                    continue;
                }
            }
            Mode::Hopper => {
                if !peer.host_name.to_lowercase().contains("hopper") {
                    // skip others
                    continue;
                }
            }
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
        info!("Zenoh connection to {:?}", zenoh_config.connect.endpoints);
    }
    if !zenoh_config.listen.endpoints.is_empty() {
        info!("Zenoh listening on {:?}", zenoh_config.listen.endpoints);
    }

    debug!("Starting zenoh session");
    let zenoh_session = zenoh::open(zenoh_config)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)?
        .into_arc();

    Ok(zenoh_session)
}
