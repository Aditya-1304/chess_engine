use crate::{
    board::Board,
    eval, movegen,
    moves::{self, Move, MoveList},
    tt::{TTFlag, TranspositionTable},
    types::{Color, PieceType},
};
use std::time::Instant;


const INF: i32 = 50000;
pub const MATE_SCORE: i32 = 49000;


pub struct Searcher {
    pub nodes: u64,
    pub start_time: Instant,
    pub time_limit_ms: u128,
    pub stop: bool,
    pub tt: TranspositionTable,
}


impl Searcher {
    pub fn new() -> Self {
        Self {
            nodes: 0,
            start_time: Instant::now(),
            time_limit_ms: 0,
            stop: false,
            tt: TranspositionTable::new(64), // 64MB default
        }
    }

    pub fn search(&mut self, board: &mut Board, depth: u8) -> (i32, Option<Move>) {
        self.nodes = 0;
        self.start_time = Instant::now();
        self.stop = false;
        if self.time_limit_ms == 0 {
            self.time_limit_ms = 5000;
        }
        let mut best_move = None;
        let mut score = 0;
        // Iterative Deepening
        for d in 1..=depth {
            let (s, m) = self.negamax(board, d, 0, -INF, INF);
            if self.stop {
                break;
            }
            score = s;
            if let Some(mv) = m {
                best_move = Some(mv);
            }
            // UCI Info output
            let time_elapsed = self.start_time.elapsed().as_millis();
            let nps = if time_elapsed > 0 { (self.nodes as u128 * 1000) / time_elapsed } else { 0 };
            
            print!("info depth {} score ", d);
            if score > 48000 {
                 let mate_in = (49000 - score + 1) / 2;
                 print!("mate {}", mate_in);
            } else if score < -48000 {
                 let mate_in = (49000 + score) / 2;
                 print!("mate -{}", mate_in);
            } else {
                 print!("cp {}", score);
            }
            println!(" nodes {} nps {} time {}", self.nodes, nps, time_elapsed);
        }
        (score, best_move)
    }


    fn negamax(
        &mut self,
        board: &mut Board,
        depth: u8,
        ply: i32,
        mut alpha: i32,
        beta: i32,
    ) -> (i32, Option<Move>) {

        const CONTEMPT: i32 = 50;

        if self.nodes & 2047 == 0 {
            if self.start_time.elapsed().as_millis() > self.time_limit_ms {
                self.stop = true;
            }
        }

        if self.stop {
            return (0, None);
        }

        if ply > 0 && (board.halfmove_clock >= 100 || board.is_repetition()) {
            return (-CONTEMPT, None);
        }

        self.nodes += 1;
        // TT Probe
        let alpha_orig = alpha;
        let mut tt_move = None;

        if let Some(entry) = self.tt.probe(board.zobrist_hash) {
            if entry.depth >= depth {
                let tt_score = score_from_tt(entry.score, ply);
                match entry.flag {
                    TTFlag::Exact => return (tt_score, entry.move_best),
                    TTFlag::Beta => {
                        if tt_score >= beta {
                            return (tt_score, entry.move_best);
                        }
                    }

                    TTFlag::Alpha => {
                        if entry.score <= alpha {
                            return (tt_score, entry.move_best);
                        }
                    }
                }
            }
            tt_move = entry.move_best;
        }

        if depth == 0 {
            return (self.quiescence(board, alpha, beta), None);
        }

        let mut move_list = MoveList::new();
        board.generate_pseudo_legal_moves(&mut move_list);

        let mut best_score = -INF;
        let mut best_move = None;
        let mut legal_moves = 0;

        for i in 0..move_list.len() {
            self.pick_move(&mut move_list, i, board, tt_move);

            let m = move_list.iter().nth(i).unwrap().clone();
            let undo = board.make_move(m);
            let us = if board.side_to_move == Color::White {
                Color::Black
            } else {
                Color::White
            };
            
            let king_sq =
                board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
                
            if board.is_square_attacked(king_sq, board.side_to_move) {
                board.unmake_move(m, undo);
                continue;
            }

            legal_moves += 1;

            let (score, _) = self.negamax(board, depth - 1, ply + 1, -beta, -alpha);

            let score = -score;
            board.unmake_move(m, undo);
            if self.stop {
                return (0, None);
            }
            if score > best_score {
                best_score = score;
                best_move = Some(m);
            }
            if score > alpha {
                alpha = score;
                best_move = Some(m); 
            }
            if alpha >= beta {
                break;
            }
        }

        if legal_moves == 0 {
            let us = board.side_to_move;
            let king_sq =
                board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
            let them = if us == Color::White {
                Color::Black
            } else {
                Color::White
            };
            if board.is_square_attacked(king_sq, them) {
                return (-MATE_SCORE + ply, None);
            } else {
                return (0, None);
            }
        }

        // TT Store
        let flag = if best_score <= alpha_orig {
            TTFlag::Alpha
        } else if best_score >= beta {
            TTFlag::Beta
        } else {
            TTFlag::Exact
        };

        let tt_score = score_to_tt(best_score, ply);
        self.tt
            .store(board.zobrist_hash, best_move, tt_score, depth, flag);
        (best_score, best_move)
    }


    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
        if self.nodes & 2047 == 0 {
            if self.start_time.elapsed().as_millis() > self.time_limit_ms {
                self.stop = true;
            }
        }
        if self.stop {
            return 0;
        }
        self.nodes += 1;
        let stand_pat = eval::evaluate(board);
        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let mut move_list = MoveList::new();
        movegen::generate_captures(board, &mut move_list);
        
        for i in 0..move_list.len() {
            self.pick_move(&mut move_list, i, board, None);
            let m = move_list.iter().nth(i).unwrap().clone();
            let undo = board.make_move(m);
            let us = if board.side_to_move == Color::White {
                Color::Black
            } else {
                Color::White
            };

            let king_sq =
                board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
            if board.is_square_attacked(king_sq, board.side_to_move) {
                board.unmake_move(m, undo);
                continue;
            }

            let score = -self.quiescence(board, -beta, -alpha);
            board.unmake_move(m, undo);
            if score >= beta {
                return beta;
            }

            if score > alpha {
                alpha = score;
            }
        }
        alpha
    }

    fn pick_move(
        &self,
        move_list: &mut MoveList,
        start_index: usize,
        board: &Board,
        tt_move: Option<Move>,
    ) {
        let moves = move_list.as_mut_slice();
        let mut best_score = -2000000000;
        let mut best_idx = start_index;
        for i in start_index..moves.len() {
            let m = moves[i];
            let mut score = 0;
            if Some(m) == tt_move {
                score = 1000000; // Highest priority
            } else if moves::is_capture(m) {
                score = self.get_mvv_lva(m, board);
            }
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }
        moves.swap(start_index, best_idx);
    }

    fn get_mvv_lva(&self, m: Move, board: &Board) -> i32 {
        let to = moves::to_sq(m);
        let from = moves::from_sq(m);
        let victim = board.piece_type_on(to).unwrap_or(PieceType::Pawn);
        let attacker = board.piece_type_on(from).unwrap();
        let vv = match victim {
            PieceType::Pawn => 1,
            PieceType::Knight => 2,
            PieceType::Bishop => 3,
            PieceType::Rook => 4,
            PieceType::Queen => 5,
            PieceType::King => 6,
        };
        let av = match attacker {
            PieceType::Pawn => 1,
            PieceType::Knight => 2,
            PieceType::Bishop => 3,
            PieceType::Rook => 4,
            PieceType::Queen => 5,
            PieceType::King => 6,
        };
        10 * vv - av + 10000
    }

    
}

fn score_to_tt(score: i32, ply: i32) -> i32 {
        if score > 48000 {
            score + ply
        } else if score < -48000 {
            score - ply
        } else {
            score
        }
    }

    fn score_from_tt(score: i32, ply: i32) -> i32 {
        if score > 48000 {
            score - ply
        } else if score < -48000 {
            score + ply
        } else {
            score
        }
    }