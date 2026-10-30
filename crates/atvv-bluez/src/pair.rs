// SPDX-License-Identifier: GPL-3.0-or-later

//! Pairing with an auto-confirm agent.
//!
//! LE Secure Connections numeric comparison presents a passkey through the
//! agent. The passkey is recorded, then accepted: the RC003 has no keyboard or
//! display, and the user has already put it in pairing mode. `NoInputNoOutput`
//! would make BlueZ reject the confirmation, so the agent advertises
//! `DisplayYesNo`.

use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bluer::agent::{Agent, ReqResult};
use bluer::{Device, Session as BluezSession};
use tokio::time::timeout;

use crate::error::{BluezError, Result};
use crate::link::select_adapter;
use crate::{build_runtime, parse_address};

/// Default time to wait for the pairing gesture and confirmation.
pub const DEFAULT_PAIR_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairConfig {
    pub address: String,
    pub adapter: Option<String>,
    pub pair_timeout: Duration,
}

impl Default for PairConfig {
    fn default() -> Self {
        Self {
            address: String::new(),
            adapter: None,
            pair_timeout: DEFAULT_PAIR_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairReport {
    pub address: String,
    pub name: Option<String>,
    /// The device was already bonded; no pairing exchange was started.
    pub already_paired: bool,
    pub paired: bool,
    pub trusted: bool,
    /// Agent callbacks seen during this run, oldest first.
    pub agent_requests: Vec<String>,
}

/// Pair, trust, and report the device state.
pub fn pair(config: PairConfig) -> Result<PairReport> {
    let runtime = build_runtime()?;
    let address = parse_address(&config.address)?;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let adapter_name = config.adapter.clone();
    let pair_timeout = config.pair_timeout;

    let report = runtime.block_on(async {
        let session = BluezSession::new().await?;
        let adapter = select_adapter(&session, adapter_name.as_deref()).await?;
        if !adapter.is_powered().await? {
            adapter.set_powered(true).await?;
        }
        let device = adapter.device(address)?;
        let already_paired = device.is_paired().await?;

        let _agent = session.register_agent(agent(Arc::clone(&requests))).await?;

        if !already_paired {
            match timeout(pair_timeout, device.pair()).await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(BluezError::Timeout {
                        operation: "pairing",
                        timeout: pair_timeout,
                    });
                }
            }
        }
        device.set_trusted(true).await?;

        Ok::<_, BluezError>(PairReport {
            address: device.address().to_string(),
            name: name(&device).await,
            already_paired,
            paired: device.is_paired().await?,
            trusted: device.is_trusted().await?,
            agent_requests: Vec::new(),
        })
    });

    let mut report = report?;
    report.agent_requests = requests
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    Ok(report)
}

async fn name(device: &Device) -> Option<String> {
    if let Ok(alias) = device.alias().await {
        if !alias.is_empty() {
            return Some(alias);
        }
    }
    device
        .name()
        .await
        .ok()
        .flatten()
        .filter(|name| !name.is_empty())
}

type AcceptFuture = Pin<Box<dyn Future<Output = ReqResult<()>> + Send>>;

fn accept() -> AcceptFuture {
    Box::pin(async { Ok(()) })
}

fn record(requests: &Mutex<Vec<String>>, message: String) {
    requests
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(message);
}

fn agent(requests: Arc<Mutex<Vec<String>>>) -> Agent {
    Agent {
        request_confirmation: Some(Box::new({
            let requests = Arc::clone(&requests);
            move |request| {
                record(
                    &requests,
                    format!(
                        "numeric comparison: passkey {:06} for {}",
                        request.passkey, request.device
                    ),
                );
                accept()
            }
        })),
        request_authorization: Some(Box::new({
            let requests = Arc::clone(&requests);
            move |request| {
                record(
                    &requests,
                    format!("authorize pairing request from {}", request.device),
                );
                accept()
            }
        })),
        authorize_service: Some(Box::new({
            let requests = Arc::clone(&requests);
            move |request| {
                record(
                    &requests,
                    format!(
                        "authorize service {} for {}",
                        request.service, request.device
                    ),
                );
                accept()
            }
        })),
        ..Agent::default()
    }
}
