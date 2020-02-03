// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// Copyright 2016 The etcd Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::VecDeque;

use crate::eraftpb::Message;
use crate::{HashMap, HashSet};

/// Determines the relative safety of and consistency of read only requests.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ReadOnlyOption {
    /// Safe guarantees the linearizability of the read only request by
    /// communicating with the quorum. It is the default and suggested option.
    Safe,
    /// LeaseBased ensures linearizability of the read only request by
    /// relying on the leader lease. It can be affected by clock drift.
    /// If the clock drift is unbounded, leader might keep the lease longer than it
    /// should (clock can move backward/pause without any bound). ReadIndex is not safe
    /// in that case.
    LeaseBased,
}

impl Default for ReadOnlyOption {
    fn default() -> ReadOnlyOption {
        ReadOnlyOption::Safe
    }
}

/// ReadState provides state for read only query.
/// It's caller's responsibility to send MsgReadIndex first before getting
/// this state from ready. It's also caller's duty to differentiate if this
/// state is what it requests through request_ctx, e.g. given a unique id as
/// request_ctx.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct ReadState {
    /// The index of the read state.
    pub index: u64,
    /// A datagram consisting of context about the request.
    pub request_ctx: Vec<u8>,
}

#[derive(Default, Debug, Clone)]
pub struct ReadIndexStatus {
    pub req: Message,
    pub index: u64,
    pub acks: HashSet<u64>,
}

#[derive(Default, Debug, Clone)]
pub struct ReadOnly {
    pub option: ReadOnlyOption,
    pub pending_read_index: HashMap<Vec<u8>, ReadIndexStatus>,
    pub read_index_queue: VecDeque<Vec<u8>>,
    // Items in `read_index_queue` with index *less* than `waiting_for_ready`
    // are pending because the peer hasn't committed to its term.
    waiting_for_ready: usize,
}

impl ReadOnly {
    pub fn new(option: ReadOnlyOption) -> ReadOnly {
        ReadOnly {
            option,
            pending_read_index: HashMap::default(),
            read_index_queue: VecDeque::new(),
            waiting_for_ready: 0,
        }
    }

    /// Adds a read only request into readonly struct.
    ///
    /// `index` is the commit index of the raft state machine when it received
    /// the read only request.
    ///
    /// `m` is the original read only request message from the local or remote node.
    pub fn add_request(&mut self, index: u64, m: Message) {
        let ctx = {
            let key = &m.entries[0].data;
            if self.pending_read_index.contains_key(key) {
                return;
            }
            key.to_vec()
        };
        let status = ReadIndexStatus {
            req: m,
            index,
            acks: HashSet::default(),
        };
        self.pending_read_index.insert(ctx.clone(), status);
        self.read_index_queue.push_back(ctx);
    }

    /// Notifies the ReadOnly struct that the raft state machine received
    /// an acknowledgment of the heartbeat that attached with the read only request
    /// context.
    pub fn recv_ack(&mut self, m: &Message) -> HashSet<u64> {
        match self.pending_read_index.get_mut(&m.context) {
            None => Default::default(),
            Some(rs) => {
                rs.acks.insert(m.from);
                // add one to include an ack from local node
                let mut set_with_self = HashSet::default();
                set_with_self.insert(m.to);
                rs.acks.union(&set_with_self).cloned().collect()
            }
        }
    }

    /// Advances the read only request queue kept by the ReadOnly struct.
    /// It dequeues the requests until it finds the read only request that has
    /// the same context as the given `m`.
    pub fn advance(&mut self, m: &Message, ready: bool) -> Vec<ReadIndexStatus> {
        let mut rss = vec![];
        if let Some(i) = self.read_index_queue.iter().position(|x| {
            debug_assert!(self.pending_read_index.contains_key(x));
            *x == m.context
        }) {
            if !ready {
                self.waiting_for_ready = std::cmp::max(self.waiting_for_ready, i + 1);
                return rss;
            }
            for _ in 0..=i {
                let rs = self.read_index_queue.pop_front().unwrap();
                let status = self.pending_read_index.remove(&rs).unwrap();
                rss.push(status);
            }
        }
        rss
    }

    pub(crate) fn advance_by_commit(&mut self, committed: u64) -> Vec<ReadIndexStatus> {
        let mut rss = vec![];
        if self.waiting_for_ready > 0 {
            let remained = self.read_index_queue.split_off(self.waiting_for_ready);
            self.waiting_for_ready = 0;
            for rs in std::mem::replace(&mut self.read_index_queue, remained) {
                let mut status = self.pending_read_index.remove(&rs).unwrap();
                // Use latest committed index to avoid stale read on follower peers.
                status.index = committed;
                rss.push(status);
            }
        }
        rss
    }

    /// Returns the context of the last pending read only request in ReadOnly struct.
    pub fn last_pending_request_ctx(&self) -> Option<Vec<u8>> {
        self.read_index_queue.back().cloned()
    }

    #[inline]
    pub fn pending_read_count(&self) -> usize {
        self.read_index_queue.len()
    }
}
