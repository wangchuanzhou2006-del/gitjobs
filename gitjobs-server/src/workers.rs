//! This module contains background workers for some tasks.

use std::{sync::Arc, time::Duration};

use chrono::Utc;
use tokio::{sync::Semaphore, task::JoinSet, time::sleep};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{db::DynDB, talent::TalentServices};

const TALENT_MAX_CONCURRENCY: usize = 4;
const TALENT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const TALENT_CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Launches all background workers.
pub(crate) fn run(
    db: DynDB,
    talent: Arc<TalentServices>,
    talent_enabled: bool,
    task_tracker: &TaskTracker,
    cancellation_token: CancellationToken,
) {
    // Jobs archiver
    let archiver_db = db.clone();
    let archiver_cancellation = cancellation_token.clone();
    task_tracker.spawn(async move {
        archiver(archiver_db, archiver_cancellation).await;
    });
    if talent_enabled {
        let processor = talent.processor.clone();
        let processor_cancellation = cancellation_token.clone();
        task_tracker.spawn(async move {
            talent_processor_worker(processor, processor_cancellation).await;
        });
        task_tracker.spawn(async move {
            talent_cleanup_worker(db, cancellation_token).await;
        });
    }
}

/// Worker that archives expired jobs periodically.
pub(crate) async fn archiver(db: DynDB, cancellation_token: CancellationToken) {
    // Random sleep to avoid multiple workers running at the same time
    tokio::select! {
        () = sleep(Duration::from_secs(rand::random_range(60..300))) => {},
        () = cancellation_token.cancelled() => return,
    }

    loop {
        // Archive expired jobs
        debug!("archiving expired jobs");
        if let Err(err) = db.archive_expired_jobs().await {
            error!("error archiving expired jobs: {err}");
        }

        // Pause for a while before the next iteration
        tokio::select! {
            () = sleep(Duration::from_hours(1)) => {},
            () = cancellation_token.cancelled() => break,
        }
    }
}

/// Polls the durable claim queue in bounded batches.
pub(crate) async fn talent_processor_worker(
    processor: Arc<crate::talent::TalentProcessor>,
    cancellation_token: CancellationToken,
) {
    let permits = Arc::new(Semaphore::new(TALENT_MAX_CONCURRENCY));
    loop {
        if cancellation_token.is_cancelled() {
            break;
        }

        let mut tasks = JoinSet::new();
        for _ in 0..TALENT_MAX_CONCURRENCY {
            let permit = permits
                .clone()
                .acquire_owned()
                .await
                .expect("talent worker semaphore is never closed");
            let processor = processor.clone();
            tasks.spawn(async move {
                let _permit = permit;
                processor.process_one().await
            });
        }

        let mut found_work = false;
        while !tasks.is_empty() {
            tokio::select! {
                () = cancellation_token.cancelled() => {
                    tasks.abort_all();
                    while tasks.join_next().await.is_some() {}
                    return;
                }
                result = tasks.join_next() => {
                    match result {
                        Some(Ok(Ok(crate::talent::ProcessOutcome::NoWork))) => {}
                        Some(Ok(Ok(_))) => found_work = true,
                        Some(Ok(Err(error))) => error!(%error, "talent item processing failed"),
                        Some(Err(_)) => error!("talent item processing task panicked"),
                        None => {}
                    }
                }
            }
        }

        if !found_work {
            tokio::select! {
                () = sleep(TALENT_POLL_INTERVAL) => {}
                () = cancellation_token.cancelled() => break,
            }
        }
    }
}

/// Removes expired encrypted source data without touching durable review records.
pub(crate) async fn talent_cleanup_worker(db: DynDB, cancellation_token: CancellationToken) {
    loop {
        debug!("cleaning expired encrypted talent source data");
        if let Err(error) = db.cleanup_talent_raw_data(Utc::now()).await {
            error!(%error, "talent raw data cleanup failed");
        }
        tokio::select! {
            () = sleep(TALENT_CLEANUP_INTERVAL) => {}
            () = cancellation_token.cancelled() => break,
        }
    }
}
