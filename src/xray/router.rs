use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use tonic::transport::{Channel, Endpoint};

pub mod proto {
    tonic::include_proto!("xray.app.router.command");
}

use proto::{
    routing_service_client::RoutingServiceClient, GetBalancerInfoRequest,
    OverrideBalancerTargetRequest,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const RPC_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalancerStatus {
    pub tag: String,
    pub running: bool,
    #[serde(rename = "override")]
    pub override_target: String,
    pub selected: Vec<String>,
}

async fn channel(api_url: &str) -> Result<RoutingServiceClient<Channel>> {
    let endpoint = Endpoint::from_shared(api_url.to_string())
        .context("invalid Xray API address")?
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(RPC_TIMEOUT);
    let channel = endpoint
        .connect()
        .await
        .context("connect to Xray gRPC API")?;
    Ok(RoutingServiceClient::new(channel))
}

pub async fn balancer_status(api_url: &str, tags: &[String]) -> Result<Vec<BalancerStatus>> {
    let mut client = channel(api_url).await?;
    let mut statuses = Vec::with_capacity(tags.len());
    for tag in tags {
        let response = client
            .get_balancer_info(GetBalancerInfoRequest { tag: tag.clone() })
            .await;
        match response {
            Ok(response) => {
                let balancer = response.into_inner().balancer;
                let override_target = balancer
                    .as_ref()
                    .and_then(|balancer| balancer.r#override.as_ref())
                    .map(|info| info.target.clone())
                    .unwrap_or_default();
                let selected = balancer
                    .and_then(|balancer| balancer.principle_target)
                    .map(|info| info.tag)
                    .unwrap_or_default();
                statuses.push(BalancerStatus {
                    tag: tag.clone(),
                    running: true,
                    override_target,
                    selected,
                });
            }
            Err(err) => {
                // One unknown balancer must not blank the whole table.
                tracing::debug!(tag, error = %err, "get balancer info failed");
                statuses.push(BalancerStatus {
                    tag: tag.clone(),
                    running: false,
                    override_target: String::new(),
                    selected: Vec::new(),
                });
            }
        }
    }
    Ok(statuses)
}

pub async fn override_balancer(api_url: &str, tag: &str, target: &str) -> Result<()> {
    let mut client = channel(api_url).await?;
    client
        .override_balancer_target(OverrideBalancerTargetRequest {
            balancer_tag: tag.to_string(),
            target: target.to_string(),
        })
        .await
        .context("override balancer target")?;
    Ok(())
}
