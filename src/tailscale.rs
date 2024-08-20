use std::collections::{HashMap, HashSet};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

impl TailscaleStatus {
    pub async fn read_from_command() -> anyhow::Result<Self> {
        let output = Command::new("tailscale")
            .arg("status")
            .arg("--json")
            .output()
            .await
            .context("failed to spawn")?;

        if !output.status.success() {
            anyhow::bail!("querying tailscale status failed");
        }

        let parsed = serde_json::from_slice(&output.stdout)?;
        Ok(parsed)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TailscaleStatus {
    #[serde(rename = "TailscaleIPs")]
    pub tailscale_ip_list: HashSet<String>,
    #[serde(rename = "Self")]
    pub self_status: TailscaleStatusSelf,
    #[serde(rename = "Peer")]
    pub peers: HashMap<String, TailscalePeer>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TailscaleStatusSelf {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    pub tailscale_ip_list: HashSet<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TailscalePeer {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    pub tailscale_ip_list: HashSet<String>,
}
