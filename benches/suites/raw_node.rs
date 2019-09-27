use criterion::{Bencher, BenchmarkId, Criterion, Throughput};
use raft::eraftpb::{ConfState, Snapshot};
use raft::{storage::MemStorage, Config, RawNode, Ready};
use std::time::{Duration, Instant};

pub fn bench_raw_node(c: &mut Criterion) {
    bench_raw_node_new(c);
    bench_raw_node_leader_propose(c);
    bench_raw_node_new_ready(c);
}

fn quick_raw_node(logger: &slog::Logger) -> RawNode<MemStorage> {
    let id = 1;
    let conf_state = ConfState::from((vec![1], vec![]));
    let storage = MemStorage::new_with_conf_state(conf_state);
    let config = Config::new(id);
    RawNode::new(&config, storage, logger).unwrap()
}

pub fn bench_raw_node_new(c: &mut Criterion) {
    let bench = |b: &mut Bencher| {
        let logger = crate::default_logger();
        b.iter(|| quick_raw_node(&logger));
    };

    c.bench_function("RawNode::new", bench);
}

pub fn bench_raw_node_leader_propose(c: &mut Criterion) {
    static KB: usize = 1024;
    let mut test_sets = vec![
        0,
        32,
        128,
        512,
        KB,
        4 * KB,
        16 * KB,
        128 * KB,
        512 * KB,
        KB * KB,
    ];
    let mut group = c.benchmark_group("RawNode::leader_propose");
    for size in test_sets.drain(..) {
        // Calculate measurement time in seconds according to the input size.
        // The approximate time might not be the best but should work fine.
        let mtime = if size < KB {
            1
        } else if size < 128 * KB {
            3
        } else {
            7
        };
        group
            .measurement_time(Duration::from_secs(mtime))
            .throughput(Throughput::Bytes(size as u64))
            .bench_with_input(
                BenchmarkId::from_parameter(size),
                &size,
                |b: &mut Bencher, size| {
                    let logger = crate::default_logger();
                    let mut node = quick_raw_node(&logger);
                    node.raft.become_candidate();
                    node.raft.become_leader();
                    b.iter_custom(|iters| {
                        let mut total = Duration::from_nanos(0);
                        for _ in 0..iters {
                            let context = vec![0; 8];
                            let value = vec![0; *size];
                            let now = Instant::now();
                            node.propose(context, value).expect("");
                            total += now.elapsed();
                        }
                        total
                    });
                },
            );
    }
}

pub fn bench_raw_node_new_ready(c: &mut Criterion) {
    c.bench_function("RawNode::ready", |b: &mut Bencher| {
        b.iter_custom(|iters| {
            let logger = crate::default_logger();
            let mut node = quick_raw_node(&logger);
            node.raft.become_candidate();
            node.raft.become_leader();
            let mut total = Duration::from_nanos(0);
            for _ in 0..iters {
                // TODO: Maybe simulate more situations. For now, just preparing a raft node after stepping a proposal
                node.propose(vec![], vec![]).expect("");
                if node.has_ready() {
                    let now = Instant::now();
                    let ready = node.ready();
                    total += now.elapsed();
                    handle_ready(&mut node, ready);
                }
            }
            total
        })
    });
}

fn handle_ready(node: &mut RawNode<MemStorage>, mut ready: Ready) {
    let store = node.raft.raft_log.store.clone();
    store
        .wl()
        .append(ready.entries())
        .expect("Persisting raft log should be successful");
    if *ready.snapshot() != Snapshot::default() {
        let s = ready.snapshot().clone();
        store
            .wl()
            .apply_snapshot(s)
            .expect("Applying snapshot should be successful");
    }
    if let Some(committed_entries) = ready.committed_entries.take() {
        if let Some(last_committed) = committed_entries.last() {
            let mut s = store.wl();
            s.mut_hard_state().commit = last_committed.index;
            s.mut_hard_state().term = last_committed.term;
        }
    }
    node.advance(ready);
}
