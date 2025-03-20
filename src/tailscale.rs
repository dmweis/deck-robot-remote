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

    pub fn get_local_ipv4_address(&self) -> anyhow::Result<std::net::IpAddr> {
        // listening address
        for local_address in &self.tailscale_ip_list {
            let address: std::net::IpAddr =
                local_address.parse().context("Failed to parse address")?;
            if address.is_ipv4() {
                return Ok(address);
            }
        }
        Ok("0.0.0.0".parse()?)
    }

    pub fn get_ipv4_address_for_device(
        &self,
        device: &str,
    ) -> anyhow::Result<Option<std::net::IpAddr>> {
        for peer in self.peers.values() {
            if !peer.host_name.to_lowercase().contains(device) {
                continue;
            }

            for local_address in &peer.tailscale_ip_list {
                let address: std::net::IpAddr =
                    local_address.parse().context("Failed to parse address")?;
                if address.is_ipv4() {
                    return Ok(Some(address));
                }
            }
        }
        Ok(None)
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
