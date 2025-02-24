use std::{pin::pin, sync::Arc};

use anyhow::{anyhow, Result};
use futures::StreamExt;
use lapin::{options::BasicConsumeOptions, types::FieldTable};
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::{global::GlobalState, transcoder::job::handle_message};

pub(crate) mod job;

pub async fn run(global: Arc<GlobalState>) -> Result<()> {
    let mut consumer = pin!(global.rmq.basic_consume(
        &global.config.transcoder.rmq_queue,
        &global.config.name,
        BasicConsumeOptions::default(),
        FieldTable::default()
    ));

    let shutdown_token = CancellationToken::new();
    let child_token = shutdown_token.child_token();
    let _drop_token = shutdown_token.drop_guard();

    loop {
        select! {
            m = consumer.next() => {
                let Some(m) = m else {
                    tracing::debug!("rmq stream closed");
                    return Err(anyhow!("rmq stream closed"));
                };

                let m = m.map_err(|e| {
                    tracing::debug!("failed to get message: {}", e);
                    anyhow!("failed to get message: {}", e)
                })?;

                tracing::debug!("got message: {:?}", m);

                tokio::spawn(handle_message(global.clone(), m, child_token.clone()));
            },
            _ = global.ctx.done() => {
                tracing::debug!("context done");
                return Ok(());
            }
        }
    }
}
