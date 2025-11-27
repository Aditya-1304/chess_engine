use crate::board::Board;
use crate::moves::Move;
use crate::search::SearchThread;
use crate::tt::TranspositionTable;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

pub struct SharedState {
    pub tt: TranspositionTable,
    pub stop: AtomicBool,
    pub nodes: AtomicU64,
}

impl SharedState {
    pub fn new(tt_size_mb: usize) -> Self {
        Self {
            tt: TranspositionTable::new(tt_size_mb),
            stop: AtomicBool::new(false),
            nodes: AtomicU64::new(0),
        }
    }
}

pub struct ThreadPool {
    pub shared: Arc<SharedState>,
    pub num_threads: usize,
}

impl ThreadPool {
    pub fn new(num_threads: usize, tt_size_mb: usize) -> Self {
        Self {
            shared: Arc::new(SharedState::new(tt_size_mb)),
            num_threads,
        }
    }

    pub fn search(
        &self,
        board: &mut Board,  // Changed: take mutable reference
        depth: u8,
        time_soft_limit: u128,
        time_hard_limit: u128,
    ) -> (i32, Option<Move>) {
        // Reset shared state
        self.shared.stop.store(false, Ordering::SeqCst);
        self.shared.nodes.store(0, Ordering::Relaxed);
        self.shared.tt.new_search();

        let mut handles = Vec::with_capacity(self.num_threads);

        // Spawn helper threads first (they will search until stopped)
        for thread_id in 1..self.num_threads {
            let shared = Arc::clone(&self.shared);
            let mut board_clone = board.clone_for_search();

            let handle = thread::spawn(move || {
                let mut search_thread = SearchThread::new(thread_id, shared, false);
                search_thread.time_soft_limit = u128::MAX;
                search_thread.time_hard_limit = u128::MAX;
                search_thread.search(&mut board_clone, depth)
            });

            handles.push(handle);
        }

        // Main thread searches directly on the board (no clone needed)
        let mut main_search = SearchThread::new(0, Arc::clone(&self.shared), true);
        main_search.time_soft_limit = time_soft_limit;
        main_search.time_hard_limit = time_hard_limit;
        
        let result = main_search.search(board, depth);  // No clone!

        // Main thread finished - stop all helper threads
        self.shared.stop.store(true, Ordering::SeqCst);

        // Now wait for helpers to finish
        for handle in handles {
            let _ = handle.join();
        }

        result
    }

    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }

    pub fn clear(&self) {
        self.shared.tt.clear();
    }

    pub fn total_nodes(&self) -> u64 {
        self.shared.nodes.load(Ordering::Relaxed)
    }
}