//! Loops that wake when something changed, not on a clock: a [`Notify`]
//! the writers poke, and a pause after each round that keeps the old rate
//! limit. Idle, such a loop costs no wake-up at all.

use std::time::Duration;
use tokio::sync::Notify;

/// Run `round` each time `changed` is notified, at most once per `every`.
/// A notification while `round` runs or during the pause after it is kept
/// (`Notify` stores one permit), so the last change is never missed, and
/// many changes in one pause make one round. Returns when `round` says
/// `false`.
pub(crate) async fn on_change(changed: &Notify, every: Duration, mut round: impl FnMut() -> bool) {
    loop {
        changed.notified().await;
        if !round() {
            return;
        }
        tokio::time::sleep(every).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(start_paused = true)]
    async fn idle_costs_nothing_and_bursts_are_coalesced() {
        let changed = Arc::new(Notify::new());
        let rounds = Arc::new(AtomicUsize::new(0));
        let task = {
            let (changed, rounds) = (Arc::clone(&changed), Arc::clone(&rounds));
            tokio::spawn(async move {
                on_change(&changed, Duration::from_millis(33), || {
                    rounds.fetch_add(1, Ordering::SeqCst);
                    true
                })
                .await;
            })
        };
        // Idle: a long time passes, no round.
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(rounds.load(Ordering::SeqCst), 0);

        // One change: one round, at once.
        changed.notify_one();
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert_eq!(rounds.load(Ordering::SeqCst), 1);

        // A burst during the pause: one more round, after the pause.
        for _ in 0..100 {
            changed.notify_one();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(rounds.load(Ordering::SeqCst), 1, "still pausing");
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(rounds.load(Ordering::SeqCst), 2);

        // Then idle again.
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(rounds.load(Ordering::SeqCst), 2);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn stops_when_the_round_says_so() {
        let changed = Notify::new();
        changed.notify_one();
        on_change(&changed, Duration::from_millis(33), || false).await;
    }
}
