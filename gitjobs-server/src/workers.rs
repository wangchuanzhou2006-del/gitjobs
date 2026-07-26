//! This module contains background workers for some tasks.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use tokio::{sync::Semaphore, task::JoinSet, time::sleep};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error};

use crate::{
    db::DynDB,
    talent::{ProcessError, ProcessOutcome, TalentServices},
};

const TALENT_MAX_CONCURRENCY: usize = 4;
const TALENT_POLL_INTERVAL: Duration = Duration::from_secs(1);
const TALENT_CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const PRIVACY_SAFE_PANIC_MESSAGE: &str = "background task panicked";

/// Replaces the default payload-printing hook before any background work starts.
pub(crate) fn install_privacy_safe_panic_hook() {
    std::panic::set_hook(Box::new(|_| eprintln!("{PRIVACY_SAFE_PANIC_MESSAGE}")));
}

#[async_trait]
trait TalentItemProcessor: Send + Sync {
    async fn process_one(&self) -> Result<ProcessOutcome, ProcessError>;
}

#[async_trait]
impl TalentItemProcessor for crate::talent::TalentProcessor {
    async fn process_one(&self) -> Result<ProcessOutcome, ProcessError> {
        crate::talent::TalentProcessor::process_one(self).await
    }
}

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
    talent_processor_worker_with(processor, cancellation_token).await;
}

async fn talent_processor_worker_with<P>(processor: Arc<P>, cancellation_token: CancellationToken)
where
    P: TalentItemProcessor + 'static,
{
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

#[cfg(test)]
mod tests {
    use std::{
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use crate::talent::{ProcessError, ProcessOutcome};

    use super::{TalentItemProcessor, talent_processor_worker_with};

    struct ActiveGuard(Arc<AtomicUsize>);

    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    struct RecoveringProcessor {
        calls: AtomicUsize,
        active: Arc<AtomicUsize>,
        maximum: AtomicUsize,
        cancellation: CancellationToken,
    }

    #[async_trait]
    impl TalentItemProcessor for RecoveringProcessor {
        async fn process_one(&self) -> Result<ProcessOutcome, ProcessError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            let _guard = ActiveGuard(self.active.clone());
            if call == 0 {
                std::panic::resume_unwind(Box::new(()));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            if call >= 4 {
                self.cancellation.cancel();
            }
            Ok(ProcessOutcome::NeedsReview)
        }
    }

    #[tokio::test]
    async fn worker_releases_panicked_permit_and_processes_a_later_batch() {
        let cancellation = CancellationToken::new();
        let processor = Arc::new(RecoveringProcessor {
            calls: AtomicUsize::new(0),
            active: Arc::new(AtomicUsize::new(0)),
            maximum: AtomicUsize::new(0),
            cancellation: cancellation.clone(),
        });

        tokio::time::timeout(
            Duration::from_secs(2),
            talent_processor_worker_with(processor.clone(), cancellation),
        )
        .await
        .expect("worker should stop after a recovered batch");

        assert!(processor.calls.load(Ordering::SeqCst) >= 5);
        assert!(processor.maximum.load(Ordering::SeqCst) <= 4);
        assert_eq!(processor.active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn installed_panic_hook_omits_payload_in_an_isolated_process() {
        let possible_payload = "synthetic-private-panic-payload-729";
        let output = Command::new(std::env::current_exe().expect("current test executable"))
            .args([
                "--exact",
                "workers::tests::privacy_safe_panic_hook_subprocess",
                "--ignored",
                "--nocapture",
            ])
            .env("GITJOBS_PANIC_HOOK_TEST_PAYLOAD", possible_payload)
            .output()
            .expect("panic hook subprocess");
        let rendered = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(!output.status.success());
        assert!(rendered.contains(super::PRIVACY_SAFE_PANIC_MESSAGE));
        assert!(!rendered.contains(possible_payload));
    }

    #[test]
    #[ignore = "subprocess helper"]
    fn privacy_safe_panic_hook_subprocess() {
        let Ok(payload) = std::env::var("GITJOBS_PANIC_HOOK_TEST_PAYLOAD") else {
            return;
        };
        super::install_privacy_safe_panic_hook();
        std::panic::panic_any(payload);
    }
}
