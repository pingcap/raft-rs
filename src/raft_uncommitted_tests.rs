// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use super::*;

fn entry(index: u64, bytes: usize) -> Entry {
    let mut entry = Entry::default();
    entry.index = index;
    entry.data = vec![0; bytes].into();
    entry
}

#[test]
fn test_unlimited_admission_still_tracks_and_releases_uncommitted_bytes() {
    let mut state = UncommittedState {
        max_uncommitted_size: NO_LIMIT as usize,
        uncommitted_size: 0,
        last_log_tail_index: 5,
    };
    let first = entry(6, 12);
    let second = entry(7, 20);
    assert!(state.maybe_increase_uncommitted_size(std::slice::from_ref(&first)));
    assert!(state.maybe_increase_uncommitted_size(std::slice::from_ref(&second)));
    assert_eq!(state.uncommitted_size, 32);
    assert!(state.maybe_increase_uncommitted_size(&[entry(8, 0)]));
    assert_eq!(state.uncommitted_size, 32);
    assert!(state.maybe_reduce_uncommitted_size(&[first]));
    assert_eq!(state.uncommitted_size, 20);
    assert!(state.maybe_reduce_uncommitted_size(&[second, entry(8, 0)]));
    assert_eq!(state.uncommitted_size, 0);
}

#[test]
fn test_finite_admission_keeps_single_large_entry_and_empty_entry_rules() {
    let mut state = UncommittedState {
        max_uncommitted_size: 12,
        uncommitted_size: 0,
        last_log_tail_index: 5,
    };
    let first = entry(6, 20);
    assert!(state.maybe_increase_uncommitted_size(std::slice::from_ref(&first)));
    assert!(!state.maybe_increase_uncommitted_size(&[entry(7, 1)]));
    assert!(state.maybe_increase_uncommitted_size(&[entry(7, 0)]));
    assert_eq!(state.uncommitted_size, 20);
    assert!(state.maybe_reduce_uncommitted_size(&[first]));
    assert!(state.maybe_increase_uncommitted_size(&[entry(8, 12)]));
    assert!(!state.maybe_increase_uncommitted_size(&[entry(9, 1)]));
    assert_eq!(state.uncommitted_size, 12);
}

#[test]
fn test_unlimited_commit_ignores_inherited_tail_and_never_wraps() {
    let mut state = UncommittedState {
        max_uncommitted_size: NO_LIMIT as usize,
        uncommitted_size: 0,
        last_log_tail_index: 5,
    };
    let current = entry(6, 12);
    assert!(state.maybe_increase_uncommitted_size(std::slice::from_ref(&current)));
    assert!(state.maybe_reduce_uncommitted_size(&[entry(4, 1000), entry(5, 1000)]));
    assert_eq!(state.uncommitted_size, 12);
    assert!(state.maybe_reduce_uncommitted_size(&[entry(5, 1000), current]));
    assert_eq!(state.uncommitted_size, 0);
    assert!(!state.maybe_reduce_uncommitted_size(&[entry(7, 1)]));
    assert_eq!(state.uncommitted_size, 0);
    assert!(state.maybe_reduce_uncommitted_size(&[]));
}
