use ptroute_trace::{run_traces_with_runner, TraceSettings, TracerouteRunner};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
struct FakeRunner {
    delays: HashMap<String, Duration>,
    counts: Arc<Mutex<HashMap<String, u32>>>,
}

impl FakeRunner {
    fn new(delays: HashMap<String, Duration>) -> Self {
        Self {
            delays,
            counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl TracerouteRunner for FakeRunner {
    fn run(&self, target: &str, _settings: &TraceSettings) -> anyhow::Result<String> {
        if let Some(delay) = self.delays.get(target) {
            thread::sleep(*delay);
        }
        let mut counts = self.counts.lock().unwrap();
        let entry = counts.entry(target.to_string()).or_insert(0);
        *entry += 1;
        Ok(format!(
            "traceroute to {0} ({0}), 30 hops max\n 1  {0}  1.0 ms",
            target
        ))
    }
}

#[test]
fn ordering_is_stable_with_concurrency() {
    let mut delays = HashMap::new();
    delays.insert("slow".to_string(), Duration::from_millis(50));
    delays.insert("fast".to_string(), Duration::from_millis(0));

    let runner = Arc::new(FakeRunner::new(delays));
    let targets = vec!["slow".to_string(), "fast".to_string()];
    let settings = TraceSettings::default();

    let results = run_traces_with_runner(&targets, &settings, 2, 0, 2, runner);

    let order: Vec<(String, u32)> = results
        .into_iter()
        .map(|job| (job.target, job.repeat))
        .collect();

    assert_eq!(
        order,
        vec![
            ("slow".to_string(), 0),
            ("slow".to_string(), 1),
            ("fast".to_string(), 0),
            ("fast".to_string(), 1)
        ]
    );
}

#[test]
fn ordering_matches_between_concurrency_levels() {
    let runner_a = Arc::new(FakeRunner::new(HashMap::new()));
    let runner_b = Arc::new(FakeRunner::new(HashMap::new()));
    let targets = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let settings = TraceSettings::default();

    let order_one: Vec<(String, u32)> =
        run_traces_with_runner(&targets, &settings, 2, 0, 1, runner_a)
            .into_iter()
            .map(|job| (job.target, job.repeat))
            .collect();

    let order_two: Vec<(String, u32)> =
        run_traces_with_runner(&targets, &settings, 2, 0, 4, runner_b)
            .into_iter()
            .map(|job| (job.target, job.repeat))
            .collect();

    assert_eq!(order_one, order_two);
}
