/*This file implemets a thread safe Transposition Table */
use crate::moves::Move;
use crate::board::ZHash;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TTFlag {
  Exact,
  Alpha, // upper bound
  Beta,  // lower bound
}


/// Atomic TT Entry using two AtomicU64s
/// Data1: key (64 bits)
/// Data2: move(16) | score(16) | depth(8) | generation(8) | flag(8) | padding(8)
#[repr(C, align(16))]
pub struct AtomicTTEntry {
    key: AtomicU64,
    data: AtomicU64,
}

impl AtomicTTEntry {
  pub fn new() -> Self {
    Self { 
      key: AtomicU64::new(0), 
      data: AtomicU64::new(0), 
    }
  }

  #[inline]
  fn pack_data(mv: u16, score: i16, depth: u8, generation: u8, flag: u8) -> u64 {
    (mv as u64)
      | ((score as u16 as u64) << 16) 
      | ((depth as u64) << 32)
      | ((generation as u64) << 40)
      | ((flag as u64) << 48)
  }

  #[inline]
  fn unpack_data(data: u64) -> (u16, i16, u8, u8, u8) {
    let mv = data as u16;
    let score = (data >> 16) as u16 as i16;
    let depth = (data >> 32) as u8;
    let generation = (data >> 40) as u8;
    let flag = (data >> 48) as u8;
    (mv, score, depth, generation, flag)
  }

  pub fn read(&self) -> Option<(ZHash, u16, i16, u8, u8, u8)> {
    let key = self.key.load(Ordering::Relaxed);
    let data = self.data.load(Ordering::Relaxed);

    if key == 0 {
      return None;
    }

    let (mv, score, depth, generation, flag) = Self::unpack_data(data);
    Some((key, mv, score, depth, generation, flag))
  }

  pub fn write(&self, key: ZHash, mv: u16, score: i16, depth: u8, generation: u8, flag: u8) {
    let data = Self::pack_data(mv, score, depth, generation, flag);
    self.key.store(key, Ordering::Relaxed);
    self.data.store(data, Ordering::Relaxed);
  }
}

/// 64-byte aligned cluster with 4 entries
#[repr(C, align(64))]
pub struct AtomicCluster {
    pub entries: [AtomicTTEntry; 4],
}

impl AtomicCluster {
  pub fn new() -> Self {
    Self {
      entries: [
        AtomicTTEntry::new(),
        AtomicTTEntry::new(),
        AtomicTTEntry::new(),
        AtomicTTEntry::new(),
      ],
     }
  }
}


pub struct TranspositionTable {
  table: Vec<AtomicCluster>,
  size: usize,
  pub generation: AtomicU8,
}

unsafe impl Send for TranspositionTable {}
unsafe impl Sync for TranspositionTable {}

impl TranspositionTable {
  pub fn new(mb_size: usize) -> Self {
    let cluster_size = std::mem::size_of::<AtomicCluster>();
    let size = (mb_size * 1024 * 1024) / cluster_size;
    let size = size.next_power_of_two();

    let mut table = Vec::with_capacity(size);
    for _ in 0..size {
      table.push(AtomicCluster::new());
    } 

    Self { table, size, generation: AtomicU8::new(0) }
  }

  pub fn new_search(&self) {
    self.generation.fetch_add(1, Ordering::Relaxed);
  }

  pub fn probe(&self, key: ZHash) -> Option<(Move, i32, u8, TTFlag)> {
    let index = (key as usize) & (self.size - 1);
    let cluster = &self.table[index];

    for i in 0..4 {
      if let Some((stored_key, mv, score, depth, _gen, flag_u8)) = cluster.entries[i].read() {
        if stored_key == key {
          let flag = match flag_u8 {
              0 => TTFlag::Exact,
              1 => TTFlag::Alpha,
              _ => TTFlag::Beta,
          };
          return Some((mv, score as i32, depth, flag));
        }
      }
    }

    None
  }

  pub fn store(
    &self,
    key: ZHash,
    move_best: Option<Move>,
    score: i32,
    depth: u8,
    flag: TTFlag,
  ) {
    let index = (key as usize) & (self.size - 1);
    let cluster = &self.table[index];
    let generation = self.generation.load(Ordering::Relaxed);

    let move_u16 = move_best.unwrap_or(0);
    let score_i16 = score.clamp(-32000, 32000) as i16;
    let flag_u8 = flag as u8;

    let mut replace_idx = 0;
    let mut found = false;
    let mut worst_score = i32::MIN;

      for i in 0..4 {
      if let Some((stored_key, _stored_mv, _, stored_depth, stored_gen, _)) =
          cluster.entries[i].read()
      {
          if stored_key == key {
              replace_idx = i;
              found = true;
              break;
          }

          // Replacement scoring: prefer old generation, then shallow depth
          let mut entry_score = 0i32;
          if stored_gen != generation {
              entry_score += 1000;
          }
          entry_score += 256 - stored_depth as i32;

          if entry_score > worst_score {
              worst_score = entry_score;
              replace_idx = i;
          }
      } else {
          // Empty slot
          replace_idx = i;
          break;
        }
      }

    // Preserve existing move if we're storing a fail-low without a move
    let final_move = if found && move_u16 == 0 {
        if let Some((_, stored_mv, _, _, _, _)) = cluster.entries[replace_idx].read() {
            stored_mv
        } else {
            move_u16
        }
    } else {
        move_u16
    };

    cluster.entries[replace_idx].write(key, final_move, score_i16, depth, generation, flag_u8);
     
  }

  pub fn clear(&self) {
    for cluster in &self.table {
      for entry in &cluster.entries {
        entry.key.store(0, Ordering::Relaxed);
        entry.data.store(0, Ordering::Relaxed);
      }
    }
    self.generation.store(0, Ordering::Relaxed);
  }
}
