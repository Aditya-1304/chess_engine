/*This file implemets a thread safe Transposition Table */
use crate::moves::Move;
use crate::board::ZHash;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TTFlag {
  Exact,
  Alpha, // upper bound
  Beta,  // lower bound
}

/// 16 bytes: 8(key) + 2(score) + 2(move) + 1(depth) + 1(gen) + 1(flag) + 1(padding)
#[derive(Clone, Copy)]
#[repr(C)]
pub struct TTEntry {
  pub key: ZHash,
  pub move_best: u16,
  pub score: i16,
  pub depth: u8,
  pub generation: u8,
  pub flag: u8,
  pub _pad: u8,
}

/// 64 bytes: 4 * 16. so that it Fits in one cache line.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct Cluster {
  pub entries: [TTEntry; 4]
}


pub struct TranspositionTable {
  table: Vec<Cluster>,
  size: usize,
  pub generation: u8,
}

impl TranspositionTable {
  pub fn new(mb_size: usize) -> Self {
    let size = (mb_size * 1024 * 1024) / std::mem::size_of::<Cluster>();
    let size = size.next_power_of_two();

    let empty_entry = TTEntry {
      key: 0,
      move_best: 0,
      score: 0,
      depth: 0,
      generation: 0,
      flag: 0,
      _pad: 0,
    };

    let empty_cluster = Cluster {
      entries: [empty_entry; 4],
    };

    Self { 
      table: vec![empty_cluster; size], 
      size, 
      generation: 0
    }
  }

  pub fn new_search(&mut self) {
    self.generation = self.generation.wrapping_add(1);
  }

  pub fn probe(&self, key: ZHash) -> Option<(Move, i32, u8, TTFlag)> {
    let index = (key as usize) & (self.size - 1);
    let cluster = &self.table[index];

    for i in 0..4 {
      let entry = &cluster.entries[i];
      if entry.key == key {
        let _mv = if entry.move_best == 0 { None } else { Some(entry.move_best) };
        let flag = match entry.flag {
            0 => TTFlag::Exact,
            1 => TTFlag::Alpha,
            _ => TTFlag::Beta,
        };
        return Some((entry.move_best, entry.score as i32, entry.depth, flag));
      }
    }
    None
  }

  pub fn store(
    &mut self,
    key: ZHash,
    move_best: Option<Move>,
    score: i32, 
    depth: u8,
    flag: TTFlag,
  ) {
    let index = (key as usize) & (self.size - 1);
    let cluster = &mut self.table[index];

    let move_u16 = move_best.unwrap_or(0);
    let score_i16 = score.clamp(-32000, 32000) as i16;
    let flag_u8 = flag as u8;

    let mut replace_idx = 0;
    let mut _min_depth = 255;
    let mut found = false;

    for i in 0..4 {
      if cluster.entries[i].key == key {
        replace_idx = i;
        found = true;
        break;
      }
    }

    if !found {
      // Find the oldest generation, then shallowest depth (not the most optimal will optimize later)
      let mut best_score = -1000;

      for i in 0..4 {
        let entry = &cluster.entries[i];
        let mut score = 0;
        if entry.generation != self.generation {
          score += 100;
        }
        if entry.depth < depth {
          score += (depth - entry.depth) as i32;
        }

        if score > best_score {
          best_score = score;
          replace_idx = i;
        }
      }
    }

    let move_to_write = if found && move_u16 == 0 {
      cluster.entries[replace_idx].move_best
    } else {
        move_u16
    };

    cluster.entries[replace_idx] = TTEntry { key,
      move_best: move_to_write, 
      score: score_i16, 
      depth, 
      generation: self.generation, 
      flag: flag_u8, 
      _pad: 0
    };
  }
    
  pub fn clear(&mut self) {
    let empty_entry = TTEntry {
      key: 0,
      move_best: 0,
      score: 0,
      depth: 0,
      generation: 0,
      flag: 0,
      _pad: 0,
    };
    let empty_cluster = Cluster {
      entries: [empty_entry; 4],
    };

    self.table.fill(empty_cluster);
    self.generation = 0;
  }
}
