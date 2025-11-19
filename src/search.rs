use crate::{board::Board, eval, moves::{Move, MoveList}, types::{Color, PieceType}};

const INF: i32 = 50000;
const MATE_SCORE: i32 = 49000;

pub struct Searcher {
  pub nodes: u64,
}

impl Searcher {
  pub fn new() -> Self {
    Self { nodes: 0 }
  }

  pub fn search(&mut self, board: &mut Board, depth: u8) -> (i32, Option<Move>) {
    self.nodes = 0;
    self.negamax(board, depth, -INF, INF)
  }

  fn negamax(
    &mut self,
    board: &mut Board,
    depth: u8,
    mut alpha: i32,
    beta: i32,
  ) -> (i32, Option<Move>) {
    self.nodes +=1;

    // Base Case: Leaf node
    if depth == 0 {
      return (eval::evaluate(board), None);
    }

    // Generate Moves
    let mut move_list = MoveList::new();
    board.generate_pseudo_legal_moves(&mut move_list);

    let mut best_score = -INF;
    let mut best_move = None;
    let mut legal_moves = 0;

    for &m in move_list.iter() {
      let undo = board.make_move(m);

      let us = if board.side_to_move == Color::White { Color::Black } else { Color::White };

      let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;

      if board.is_square_attacked(king_sq, board.side_to_move) {
        board.unmake_move(m, undo);
        continue;
      }

      legal_moves += 1;

      let (score, _) = self.negamax(board, depth - 1, -beta, -alpha);
      let score = -score;

      board.unmake_move(m, undo);

      // Found a better move?
      if score > best_score {
        best_score = score;
        best_move = Some(m);
      }

      // Alpha-Beta Pruning
      if score > alpha {
        alpha = score;
      }

      if alpha >= beta {
        break;
      }
    }

    // Checkmate / stalemate
    if legal_moves == 0 {
      let us = board.side_to_move;
      let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;

      let them = if us == Color::White { Color:: Black } else { Color::White };

      if board.is_square_attacked(king_sq, them) {
        return  (-MATE_SCORE + (depth as i32), None);
      } else {
        return (0, None);
      }
    }

    (best_score, best_move)

  }
}