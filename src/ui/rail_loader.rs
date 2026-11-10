//! Bounded per-rail scheduling, independent of GTK and transport.
pub const RAILS: [&str; 3] = ["sale", "new", "free"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Job {
    pub rail: usize,
    pub generation: u64,
    pub force: bool,
}

#[derive(Default)]
pub struct Scheduler {
    generation: u64,
    running: [Option<Job>; 3],
    pending: [Option<Job>; 3],
}

impl Scheduler {
    pub fn request(&mut self, force: bool) -> Vec<Job> {
        self.generation = self.generation.wrapping_add(1);
        let mut starts = Vec::new();
        for rail in 0..RAILS.len() {
            let job = Job {
                rail,
                generation: self.generation,
                force: force || self.pending[rail].is_some_and(|job| job.force),
            };
            if self.running[rail].is_some() {
                self.pending[rail] = Some(job);
            } else {
                self.running[rail] = Some(job);
                starts.push(job);
            }
        }
        starts
    }

    pub fn complete(&mut self, job: Job) -> (bool, Option<Job>) {
        if self.running[job.rail] != Some(job) {
            return (false, None);
        }
        let next = self.pending[job.rail].take();
        self.running[job.rail] = next;
        (job.generation == self.generation, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn fast_success_and_failure_arrive_before_held_slow_rail() {
        let mut scheduler = Scheduler::default();
        let jobs = scheduler.request(false);
        let (tx, rx) = mpsc::channel();
        let (release, held) = mpsc::channel();
        let slow_tx = tx.clone();
        let slow_job = jobs[0];
        let slow = std::thread::spawn(move || {
            held.recv().unwrap();
            slow_tx.send((slow_job, Ok("sale"))).unwrap();
        });
        let workers: Vec<_> = jobs[1..]
            .iter()
            .copied()
            .map(|job| {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let result = if job.rail == 1 {
                        Ok("new")
                    } else {
                        Err("offline")
                    };
                    tx.send((job, result)).unwrap();
                })
            })
            .collect();
        let mut successes = 0;
        let mut failures = 0;
        for _ in 0..2 {
            let (job, result) = rx.recv().unwrap();
            assert_ne!(job.rail, 0);
            assert_eq!(scheduler.complete(job), (true, None));
            if result.is_ok() {
                successes += 1;
            } else {
                failures += 1;
            }
        }
        assert_eq!((successes, failures), (1, 1));
        release.send(()).unwrap();
        let (job, result) = rx.recv().unwrap();
        assert!(result.is_ok());
        assert_eq!(scheduler.complete(job), (true, None));
        slow.join().unwrap();
        for worker in workers {
            worker.join().unwrap();
        }
    }

    #[test]
    fn refreshes_coalesce_keep_force_and_drop_old_generations() {
        let mut scheduler = Scheduler::default();
        let original = scheduler.request(false);
        assert!(scheduler.request(true).is_empty());
        for _ in 0..100 {
            assert!(scheduler.request(false).is_empty());
            assert_eq!(scheduler.running.iter().flatten().count(), 3);
            assert_eq!(scheduler.pending.iter().flatten().count(), 3);
        }
        for job in original {
            let (publish, next) = scheduler.complete(job);
            assert!(!publish);
            let next = next.unwrap();
            assert_eq!(next.rail, job.rail);
            assert_eq!(next.generation, scheduler.generation);
            assert!(next.force);
            assert_eq!(scheduler.complete(job), (false, None));
            assert_eq!(scheduler.complete(next), (true, None));
        }
        assert_eq!(scheduler.running.iter().flatten().count(), 0);
    }

    #[test]
    fn new_refresh_only_starts_idle_rails() {
        let mut scheduler = Scheduler::default();
        let first = scheduler.request(false);
        assert_eq!(scheduler.complete(first[1]), (true, None));
        let starts = scheduler.request(true);
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].rail, 1);
        assert_eq!(scheduler.running.iter().flatten().count(), 3);
        assert!(!scheduler.complete(first[0]).0);
    }
}
