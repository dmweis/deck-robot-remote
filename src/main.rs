mod messages;

use anyhow::Context;
use clap::Parser;
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use gilrs::GilrsBuilder;
use schemars::schema_for;
use thiserror::Error;
use tracing::*;
use zenoh::{config::Config, prelude::r#async::*};

use crate::messages::{Button, InputMessage};

#[derive(Parser)]
#[command(author, version)]
struct Args {
    /// The key expression to publish onto.
    #[clap(short, long, default_value = "remote-control/gamepad")]
    topic: String,

    /// Endpoints to connect to.
    #[clap(short = 'e', long)]
    connect: Vec<zenoh_config::EndPoint>,

    /// Endpoints to listen on.
    #[clap(long)]
    listen: Vec<zenoh_config::EndPoint>,

    /// A configuration file.
    #[clap(short, long)]
    config: Option<String>,

    /// Loop sleep time
    #[clap(short, long, default_value = "50")]
    sleep_ms: u64,

    /// verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
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

#[derive(Error, Debug)]
pub enum ErrorWrapper {
    #[error("Zenoh error {0:?}")]
    ZenohError(#[from] zenoh::Error),
}

#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    let args: Args = Args::parse();
    setup_tracing(args.verbose);

    let mut zenoh_config = if let Some(conf_file) = &args.config {
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

    // log config
    if let Some(config) = &args.config {
        info!("Using zenoh config {:?}", config);
    }
    if !zenoh_config.connect.endpoints.is_empty() {
        info!("Connecting to {:?}", zenoh_config.connect.endpoints);
    }
    if !zenoh_config.listen.endpoints.is_empty() {
        info!("Listening on {:?}", zenoh_config.listen.endpoints);
    }
    info!("Publishing on topic {:?}", args.topic);
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

    start_schema_queryable(zenoh_session.clone(), &args.topic).await?;

    let gamepad_publisher = zenoh_session
        .declare_publisher(args.topic)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)?;

    info!("Starting gamepad reader");

    // gamepad
    let mut gilrs = GilrsBuilder::new()
        .with_default_filters(true)
        .build()
        .expect("Failed to get gilrs handle");

    info!("{} gamepad(s) found", gilrs.gamepads().count());
    for (_id, gamepad) in gilrs.gamepads() {
        info!("{} is {:?}", gamepad.name(), gamepad.power_info());
    }

    let mut message_data = InputMessage {
        gamepads: HashMap::new(),
        time: std::time::SystemTime::now().into(),
    };

    let keep_running = Arc::new(AtomicBool::new(true));

    ctrlc::set_handler({
        let keep_running = keep_running.clone();
        move || {
            keep_running.store(false, Ordering::SeqCst);
        }
    })
    .context("Error setting Ctrl-C handler")?;

    while keep_running.load(Ordering::SeqCst) {
        let loop_start = tokio::time::Instant::now();
        while let Some(gilrs_event) = gilrs.next_event() {
            let gamepad_id: usize = gilrs_event.id.into();
            let gamepad_data = message_data.gamepads.entry(gamepad_id).or_default();

            gamepad_data.last_event_time = std::time::SystemTime::now().into();
            match gilrs_event.event {
                gilrs::EventType::ButtonPressed(button, _) => {
                    *gamepad_data
                        .button_down_event_counter
                        .entry(button.into())
                        .or_default() += 1;
                }
                gilrs::EventType::ButtonReleased(button, _) => {
                    *gamepad_data
                        .button_up_event_counter
                        .entry(button.into())
                        .or_default() += 1;
                }
                gilrs::EventType::AxisChanged(axis, value, _) => {
                    gamepad_data.axis_state.insert(axis.into(), value);
                }
                gilrs::EventType::Connected => {
                    gamepad_data.connected = true;
                    info!("Gamepad {} - {} connected", gamepad_id, gamepad_data.name)
                }
                gilrs::EventType::Disconnected => {
                    gamepad_data.connected = false;
                    warn!(
                        "Gamepad {} - {} disconnected",
                        gamepad_id, gamepad_data.name
                    )
                }
                _ => {}
            }
        }

        let mut known_ids = vec![];

        for (gamepad_id, gamepad) in gilrs.gamepads() {
            let gamepad_id: usize = gamepad_id.into();
            known_ids.push(gamepad_id);
            let gamepad_data = message_data.gamepads.entry(gamepad_id).or_default();

            gamepad_data.connected = gamepad.is_connected();
            gamepad_data.name = gamepad.name().to_string();

            if gamepad.is_connected() {
                for button in Button::all_gilrs_buttons() {
                    gamepad_data
                        .button_down
                        .insert(Button::from(*button), gamepad.is_pressed(*button));
                }

                // should we also get stick values here or use events?
                // let x = gamepad.value(gilrs::Axis::LeftStickY);
                // let x = if x.abs() > 0.2 { x } else { 0.0 };
            }
        }

        // remove gamepads that are no longer connected
        message_data
            .gamepads
            .retain(|gamepad_id, _| known_ids.contains(gamepad_id));

        message_data.time = std::time::SystemTime::now().into();
        let json = serde_json::to_string(&message_data)?;
        gamepad_publisher
            .put(json)
            .res()
            .await
            .map_err(ErrorWrapper::ZenohError)?;
        tokio::time::sleep_until(loop_start + Duration::from_millis(args.sleep_ms)).await;
    }

    Ok(())
}

async fn start_schema_queryable(
    zenoh_session: Arc<Session>,
    pub_topic: &str,
) -> anyhow::Result<()> {
    let schema_topic = format!("{}/__schema__", pub_topic);

    let queryable = zenoh_session
        .declare_queryable(&schema_topic)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)?;

    tokio::spawn(async move {
        while let Ok(query) = queryable.recv_async().await {
            let schema = schema_for!(InputMessage);
            if let Ok(schema) = serde_json::to_string(&schema) {
                if let Ok(key_expr) = KeyExpr::<'static>::from_str(&schema_topic) {
                    let reply = Ok(Sample::new(key_expr, schema));
                    _ = query.reply(reply).res().await;
                }
            }
        }
    });

    Ok(())
}
