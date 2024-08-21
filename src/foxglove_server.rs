use anyhow::Context;
use foxglove_ws::{Channel, FoxgloveWebSocket};
use prost_reflect::MessageDescriptor;
use serde::Deserialize;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::info;
use zenoh::prelude::r#async::*;

use crate::{error::ErrorWrapper, DESCRIPTOR_POOL};

pub fn create_foxglove_url() -> String {
    String::from("https://app.foxglove.dev/david-weis/view?ds=foxglove-websocket&ds.url=ws://127.0.0.1:8765/&layoutId=ea22e72c-f654-4743-925a-7143a510d390")
}

fn json_schema_table() -> &'static HashMap<String, String> {
    static INSTANCE: OnceLock<HashMap<String, String>> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert(
            "GENERIC_JSON_SCHEMA".to_owned(),
            GENERIC_JSON_SCHEMA.to_owned(),
        );
        m.insert(
            "IKEA_DIMMER_JSON_SCHEMA".to_owned(),
            IKEA_DIMMER_JSON_SCHEMA.to_owned(),
        );
        m.insert(
            "MOTION_SENSOR_JSON_SCHEMA".to_owned(),
            MOTION_SENSOR_JSON_SCHEMA.to_owned(),
        );
        m.insert(
            "CONTACT_SENSOR_JSON_SCHEMA".to_owned(),
            CONTACT_SENSOR_JSON_SCHEMA.to_owned(),
        );
        m.insert(
            "CLIMATE_SENSOR_JSON_SCHEMA".to_owned(),
            CLIMATE_SENSOR_JSON_SCHEMA.to_owned(),
        );
        m.insert(
            "VOICE_PROBABILITY_JSON_SCHEMA".to_owned(),
            VOICE_PROBABILITY_JSON_SCHEMA.to_owned(),
        );
        m
    })
}

pub async fn start_foxglove_bridge(
    config: Configuration,
    host: SocketAddr,
    zenoh_session: Arc<Session>,
) -> anyhow::Result<()> {
    // start foxglove server
    let server = foxglove_ws::FoxgloveWebSocket::new();
    tokio::spawn({
        let server = server.clone();
        async move { server.serve(host).await }
    });

    for proto_subscription in &config.protobuf_subscriptions {
        let message_descriptor = DESCRIPTOR_POOL
            .get_message_by_name(&proto_subscription.proto_type)
            .context("Failed to find protobuf message descriptor by name")?;

        start_proto_subscriber_from_descriptor(
            &proto_subscription.topic,
            zenoh_session.clone(),
            &server,
            &message_descriptor,
        )
        .await?;
    }

    for json_subscription in &config.json_subscriptions {
        info!(?json_subscription, "Starting json subscription");
        let json_schema = if let Some(json_schema_name) = &json_subscription.json_schema_name {
            json_schema_table()
                .get(json_schema_name)
                .context("Failed to load json schema")?
        } else {
            GENERIC_JSON_SCHEMA
        };

        let latched = json_subscription.latched.unwrap_or(false);

        start_json_subscriber(
            &json_subscription.topic,
            zenoh_session.clone(),
            &server,
            &json_subscription.type_name,
            json_schema,
            latched,
        )
        .await?;
    }

    Ok(())
}

async fn start_proto_subscriber_from_descriptor(
    topic: &str,
    zenoh_session: Arc<Session>,
    foxglove_server: &FoxgloveWebSocket,
    protobuf_descriptor: &MessageDescriptor,
) -> anyhow::Result<()> {
    info!(topic, "Starting proto subscriber");
    let zenoh_subscriber = zenoh_session
        .declare_subscriber(topic)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)?;

    let foxglove_channel =
        create_publisher_for_protobuf_descriptor(protobuf_descriptor, foxglove_server, topic)
            .await?;

    tokio::spawn({
        let topic = topic.to_owned();
        async move {
            let mut message_counter = 0;
            loop {
                let res: anyhow::Result<()> = async {
                    let sample = zenoh_subscriber.recv_async().await?;
                    message_counter += 1;
                    let now = SystemTime::now();
                    let time_nanos = system_time_to_nanos(&now);
                    let payload: Vec<u8> = sample.value.try_into()?;
                    foxglove_channel.send(time_nanos, &payload).await?;

                    if message_counter % 20 == 0 {
                        info!(
                            topic,
                            message_counter, "{} sent {} messages", topic, message_counter
                        );
                    }
                    Ok(())
                }
                .await;
                if let Err(err) = res {
                    tracing::error!(topic, "Error receiving message: {}", err);
                }
            }
        }
    });
    Ok(())
}

const PROTOBUF_ENCODING: &str = "protobuf";

async fn create_publisher_for_protobuf_descriptor(
    protobuf_descriptor: &MessageDescriptor,
    foxglove_server: &FoxgloveWebSocket,
    topic: &str,
) -> anyhow::Result<Channel> {
    let protobuf_schema_data = protobuf_descriptor.parent_pool().encode_to_vec();
    foxglove_server
        .create_publisher(
            topic,
            PROTOBUF_ENCODING,
            protobuf_descriptor.full_name(),
            protobuf_schema_data,
            Some(PROTOBUF_ENCODING),
            false,
        )
        .await
}

const JSON_ENCODING: &str = "json";

async fn start_json_subscriber(
    topic: &str,
    zenoh_session: Arc<Session>,
    foxglove_server: &FoxgloveWebSocket,
    type_name: &str,
    json_schema: &str,
    latched: bool,
) -> anyhow::Result<()> {
    info!(topic, "Starting json subscriber");
    let zenoh_subscriber = zenoh_session
        .declare_subscriber(topic)
        .res()
        .await
        .map_err(ErrorWrapper::ZenohError)?;
    let foxglove_channel = foxglove_server
        .create_publisher(
            topic,
            JSON_ENCODING,
            type_name,
            json_schema,
            Some("jsonschema"),
            latched,
        )
        .await?;

    tokio::spawn({
        let topic = topic.to_owned();
        async move {
            let mut message_counter = 0;
            loop {
                let res: anyhow::Result<()> = async {
                    let sample = zenoh_subscriber.recv_async().await?;
                    message_counter += 1;
                    let now = SystemTime::now();
                    let time_nanos = system_time_to_nanos(&now);

                    let payload = match &sample.encoding {
                        Encoding::Exact(KnownEncoding::TextPlain) => {
                            let payload: String = sample.value.try_into()?;
                            payload.as_bytes().to_vec()
                        }
                        Encoding::Exact(KnownEncoding::TextJson) => {
                            let payload: String = sample.value.try_into()?;
                            payload.as_bytes().to_vec()
                        }
                        Encoding::Exact(KnownEncoding::AppOctetStream) => {
                            let payload: Vec<u8> = sample.value.try_into()?;
                            payload
                        }
                        _ => {
                            tracing::error!(topic, "Unknown encoding: {:?}", sample.encoding);
                            panic!("Unknown encoding");
                        }
                    };

                    foxglove_channel.send(time_nanos, &payload).await?;

                    if message_counter % 20 == 0 {
                        info!(
                            topic,
                            message_counter, "{} sent {} messages", topic, message_counter
                        );
                    }
                    Ok(())
                }
                .await;
                if let Err(err) = res {
                    tracing::error!(topic, "Error receiving message: {}", err);
                }
            }
        }
    });
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct Configuration {
    pub protobuf_subscriptions: Vec<ProtobufSubscription>,
    pub json_subscriptions: Vec<JsonSubscription>,
}

#[derive(Debug, Deserialize)]
pub struct ProtobufSubscription {
    pub topic: String,
    pub proto_type: String,
}

#[derive(Debug, Deserialize)]
pub struct JsonSubscription {
    pub topic: String,
    pub type_name: String,
    pub json_schema_name: Option<String>,
    pub latched: Option<bool>,
}

pub fn system_time_to_nanos(d: &SystemTime) -> u64 {
    let ns = d.duration_since(UNIX_EPOCH).unwrap().as_nanos();
    assert!(ns <= u64::MAX as u128);
    ns as u64
}

#[allow(dead_code)]
const GENERIC_JSON_SCHEMA: &str = r#"
{
"title": "GenericJsonSchema",
"description": "Generic JSON Schema",
"type": "object",
"properties": {}
}
"#;

const IKEA_DIMMER_JSON_SCHEMA: &str = r#"
{
    "$schema": "http://json-schema.org/draft-04/schema#",
    "type": "object",
    "properties": {
      "action": {
        "type": "string"
      },
      "battery": {
        "type": "integer"
      },
      "brightness": {
        "type": "integer"
      },
      "linkquality": {
        "type": "integer"
      }
    },
    "required": [
      "action",
      "battery",
      "brightness",
      "linkquality"
    ]
}
"#;

const MOTION_SENSOR_JSON_SCHEMA: &str = r#"
{
    "$schema": "http://json-schema.org/draft-04/schema#",
    "type": "object",
    "properties": {
      "battery": {
        "type": "integer"
      },
      "battery_low": {
        "type": "boolean"
      },
      "linkquality": {
        "type": "integer"
      },
      "occupancy": {
        "type": "boolean"
      },
      "tamper": {
        "type": "boolean"
      },
      "voltage": {
        "type": "integer"
      }
    },
    "required": [
      "battery",
      "battery_low",
      "linkquality",
      "occupancy",
      "tamper",
      "voltage"
    ]
  }
"#;

const CONTACT_SENSOR_JSON_SCHEMA: &str = r#"
{
"$schema": "http://json-schema.org/draft-04/schema#",
"type": "object",
"properties": {
    "battery": {
    "type": "integer"
    },
    "battery_low": {
    "type": "boolean"
    },
    "contact": {
    "type": "boolean"
    },
    "linkquality": {
    "type": "integer"
    },
    "tamper": {
    "type": "boolean"
    },
    "voltage": {
    "type": "integer"
    }
},
"required": [
    "battery",
    "battery_low",
    "contact",
    "linkquality",
    "tamper",
    "voltage"
]
}
"#;

const CLIMATE_SENSOR_JSON_SCHEMA: &str = r#"
{
"$schema": "http://json-schema.org/draft-04/schema#",
"type": "object",
"properties": {
    "battery": {
    "type": "integer"
    },
    "humidity": {
    "type": "number"
    },
    "linkquality": {
    "type": "integer"
    },
    "temperature": {
    "type": "number"
    },
    "voltage": {
    "type": "integer"
    }
},
"required": [
    "battery",
    "humidity",
    "linkquality",
    "temperature",
    "voltage"
]
}
"#;

const VOICE_PROBABILITY_JSON_SCHEMA: &str = r#"
{
    "$schema": "http://json-schema.org/draft-04/schema#",
    "type": "object",
    "properties": {
      "probability": {
        "type": "number"
      },
      "timestamp": {
        "type": "string"
      }
    },
    "required": [
      "probability",
      "timestamp"
    ]
  }
"#;
