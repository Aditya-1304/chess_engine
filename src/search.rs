use crate::{
    board::Board, 
    book::OpeningBook, 
    eval, 
    movegen, 
    moves::{
        self, 
        Move, 
        MoveList
    }, 
    tt::{
        TTFlag, 
        TranspositionTable
    }, 
    types::{
        Color, 
        PieceType
    }
};
use std::time::Instant;


const INF: i32 = 32000;
pub const MATE_SCORE: i32 = 31000;


pub struct Searcher {
    pub nodes: u64,
    pub start_time: Instant,
    pub time_limit_ms: u128,
    pub stop: bool,
    pub tt: TranspositionTable,
    pub book: OpeningBook,
    pub killers: [[Option<Move>; 2]; 64],
    pub history: [[[i32; 64]; 2]; 6],
}


impl Searcher {
    pub fn new() -> Self {
        let book = OpeningBook::new("Perfect2023.bin");
        if book.file.is_some() {
            println!("info string Opening book loaded successfully");
        } else {
            println!("info string Warning: book.bin not found");
        }

        let keys = crate::zobrist::keys();
        println!("--- DEBUG ZOBRIST KEYS ---");
        // 1. Check the first key (White Pawn on A1) - Should match POLYGOT_RANDOM[0]
        // A1 is square 0. White is 0. Pawn is 0.
        println!("White Pawn A1 (Index 0):   {:016x}", keys.pieces[0][0][0]);
        
        // 2. Check the last piece key (Black King on H8) - Should match POLYGOT_RANDOM[767]
        // H8 is 63. Black is 1. King is 5.
        // Offset = 64 * (2 * 5 + 1) = 704. 704 + 63 = 767.
        println!("Black King H8 (Index 767): {:016x}", keys.pieces[5][1][63]);
        
        // 3. Check Castling Key (White King Side) - Should match POLYGOT_RANDOM[768]
        // Castling right 1 (WK)
        println!("Castle WK (Index 768):     {:016x}", keys.castling[1]);
        
        println!("--------------------------");
        Self {
            nodes: 0,
            start_time: Instant::now(),
            time_limit_ms: 0,
            stop: false,
            tt: TranspositionTable::new(64), // 64MB default
            book,
            killers: [[None; 2]; 64],
            history: [[[0; 64]; 2]; 6]
        }
    }

    pub fn search(&mut self, board: &mut Board, depth: u8) -> (i32, Option<Move>) {
        self.nodes = 0;
        self.start_time = Instant::now();
        self.stop = false;
        self.tt.new_search();
        self.killers = [[None; 2]; 64];

        if self.time_limit_ms == 0 {
            self.time_limit_ms = 5000;
        }

        let mut best_move = None;
        let mut score = 0;


        println!("info string Zobrist Hash: {:x}", board.zobrist_hash);
        if let Some(book_move) = self.book.get_move(board.zobrist_hash) {
            let mut move_list = MoveList::new();
            board.generate_pseudo_legal_moves(&mut move_list);

            let mut found_move = None;

            for &m in move_list.iter() {
                if moves::from_sq(m) == moves::from_sq(book_move)
                && moves::to_sq(m) == moves::to_sq(book_move) {
                    if moves::is_promotion(book_move) {
                        if moves::is_promotion(m) && moves::promotion_piece(m) == moves::promotion_piece(book_move) {
                            found_move = Some(m);
                            break;
                        }
                    } else {
                        found_move = Some(m);
                        break;
                    }
                }

            }
            if let Some(real_move) = found_move {
                
            return (0, Some(real_move));
            } else {

            }
            
        }

        // Iterative Deepening
        for d in 1..=depth {
            let (s, m) = self.negamax(board, d, 0, -INF, INF, true);
            if self.stop {
                break;
            }

            score = s;
            if let Some(mv) = m {
                best_move = Some(mv);
            }

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
            
            print!(" pv");
            let mut pv_board = board.clone();
            for _ in 0..d {
                if let Some((mv, _, _, _)) = self.tt.probe(pv_board.zobrist_hash) {
                    if mv != 0 {
                        print!(" {}", moves::format(mv));
                        pv_board.make_move(mv);
                    } else { break; }
                } else {
                    break;
                }
            }
            println!(" nodes {} nps {} time {}", self.nodes, nps , time_elapsed);
        }
        (score, best_move)
    }


    fn negamax(
        &mut self,
        board: &mut Board,
        mut depth: u8,
        ply: i32,
        mut alpha: i32,
        beta: i32,
        do_null: bool,
    ) -> (i32, Option<Move>) {
        if self.nodes & 2047 == 0 {
            if self.start_time.elapsed().as_millis() > self.time_limit_ms {
                self.stop = true;
            }
        }
        if self.stop { return (0, None); }

        let is_root = ply == 0;
        if !is_root && (board.halfmove_clock >= 100 || board.is_repetition()) {
            return (0, None);
        }

        if depth == 0 {
            return (self.quiescence(board, alpha, beta), None);
        }

        self.nodes += 1;

        let mut tt_move = None;
        if let Some((mv, sc, d, flag)) = self.tt.probe(board.zobrist_hash) {
            tt_move = if mv != 0 { Some(mv) } else { None };
            if !is_root && d >= depth {
                let tt_score = score_from_tt(sc, ply);
                match flag {
                    TTFlag::Exact => return (tt_score, tt_move),
                    TTFlag::Beta => {
                        if tt_score >= beta { return (tt_score, tt_move); }
                    }
                    TTFlag::Alpha => {
                        if tt_score <= alpha { return (tt_score, tt_move); }
                    }
                }
            }
        }

        let in_check = board.is_square_attacked(
            board.pieces[PieceType::King as usize][board.side_to_move as usize].trailing_zeros() as u8,
            if board.side_to_move == Color::White { Color::Black } else { Color::White }
        );

        if in_check { depth += 1; }

        // Null Move Pruning
        if do_null && !in_check && !is_root && depth >= 3 {
            let static_eval = eval::evaluate(board);
            if static_eval >= beta {
                let r = 2;
                let old_ep = board.make_null_move();
                let (score, _) = self.negamax(board, depth - 1 - r, ply + 1, -beta, -beta + 1, false);
                board.unmake_null_move(old_ep);
                if -score >= beta { return (beta, None); }
            }
        }

        let mut move_list = MoveList::new();
        board.generate_pseudo_legal_moves(&mut move_list);

        // Score moves ONCE
        let mut move_scores = [0; 256];
        for i in 0..move_list.len() {
            let m = move_list.iter().nth(i).unwrap().clone();
            if Some(m) == tt_move {
                move_scores[i] = 2000000000;
            } else if moves::is_capture(m) {
                move_scores[i] = 1000000 + self.get_mvv_lva(m, board);
            } else {
                if ply < 64 { // Check bounds for reading
                    if self.killers[ply as usize][0] == Some(m) {
                        move_scores[i] = 900000;
                    } else if self.killers[ply as usize][1] == Some(m) {
                        move_scores[i] = 800000;
                    }
                }
                if move_scores[i] == 0 {
                    let pt = board.piece_type_on(moves::from_sq(m)).unwrap();
                    let c = board.side_to_move;
                    let to = moves::to_sq(m);
                    move_scores[i] = self.history[pt as usize][c as usize][to as usize];
                }
            }
        }

        let mut best_score = -INF;
        let mut best_move = None;
        let mut legal_moves = 0;
        let alpha_orig = alpha;

        for i in 0..move_list.len() {
            // Selection Sort
            let mut best_pick_score = -2000000000;
            let mut best_pick_idx = i;
            for j in i..move_list.len() {
                if move_scores[j] > best_pick_score {
                    best_pick_score = move_scores[j];
                    best_pick_idx = j;
                }
            }
            // Swap moves AND scores
            {
                let moves_slice = move_list.as_mut_slice();
                moves_slice.swap(i, best_pick_idx);
                move_scores.swap(i, best_pick_idx);
            }

            let m = move_list.iter().nth(i).unwrap().clone();
            let undo = board.make_move(m);
            
            let us = if board.side_to_move == Color::White { Color::Black } else { Color::White };
            let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
            if board.is_square_attacked(king_sq, board.side_to_move) {
                board.unmake_move(m, undo);
                continue;
            }
            legal_moves += 1;

            let mut score;
            if legal_moves == 1 {
                let (s, _) = self.negamax(board, depth - 1, ply + 1, -beta, -alpha, true);
                score = -s;
            } else {
                let mut reduction = 0;
                if depth >= 3 && legal_moves > 4 && !moves::is_capture(m) && !moves::is_promotion(m) && !in_check {
                    reduction = 1;
                    if legal_moves > 10 { reduction = 2; }
                }
                let (s, _) = self.negamax(board, depth - 1 - reduction, ply + 1, -alpha - 1, -alpha, true);
                score = -s;
                if score > alpha && reduction > 0 {
                     let (s, _) = self.negamax(board, depth - 1, ply + 1, -alpha - 1, -alpha, true);
                     score = -s;
                }
                if score > alpha && score < beta {
                     let (s, _) = self.negamax(board, depth - 1, ply + 1, -beta, -alpha, true);
                     score = -s;
                }
            }

            board.unmake_move(m, undo);
            if self.stop { return (0, None); }

            if score > best_score {
                best_score = score;
                best_move = Some(m);
                if score > alpha {
                    alpha = score;
                    if !moves::is_capture(m) {
                        let pt = board.piece_type_on(moves::from_sq(m)).unwrap();
                        let c = us;
                        let to = moves::to_sq(m);
                        self.history[pt as usize][c as usize][to as usize] += (depth as i32) * (depth as i32);
                        if self.history[pt as usize][c as usize][to as usize] > 20000 {
                             self.history[pt as usize][c as usize][to as usize] /= 2;
                        }
                        if ply < 64 && self.killers[ply as usize][0] != Some(m) { // CHANGED: Added ply < 64 check
                            self.killers[ply as usize][1] = self.killers[ply as usize][0];
                            self.killers[ply as usize][0] = Some(m);
                        }
                    }
                }
            }
            if alpha >= beta {
                if !moves::is_capture(m) {
                     let pt = board.piece_type_on(moves::from_sq(m)).unwrap();
                     let c = us;
                     let to = moves::to_sq(m);
                     self.history[pt as usize][c as usize][to as usize] += (depth as i32) * (depth as i32);
                     if ply < 64 && self.killers[ply as usize][0] != Some(m) { // CHANGED: Added ply < 64 check
                        self.killers[ply as usize][1] = self.killers[ply as usize][0];
                        self.killers[ply as usize][0] = Some(m);
                    }
                }
                break;
            }
        }

        if legal_moves == 0 {
            if in_check { return (-MATE_SCORE + ply, None); } else { return (0, None); }
        }

        let flag = if best_score <= alpha_orig { TTFlag::Alpha } else if best_score >= beta { TTFlag::Beta } else { TTFlag::Exact };
        self.tt.store(board.zobrist_hash, best_move, score_to_tt(best_score, ply), depth, flag);
        (best_score, best_move)
    }


    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
        if self.nodes & 2047 == 0 {
             if self.start_time.elapsed().as_millis() > self.time_limit_ms { self.stop = true; }
        }
        if self.stop { return 0; }
        self.nodes += 1;
        
        let stand_pat = eval::evaluate(board);
        if stand_pat >= beta { return beta; }
        if stand_pat > alpha { alpha = stand_pat; }

        let mut move_list = MoveList::new();
        movegen::generate_captures(board, &mut move_list);

        // Simple sort for qsearch
        let mut move_scores = [0; 256];
        for i in 0..move_list.len() {
            let m = move_list.iter().nth(i).unwrap().clone();
            move_scores[i] = 1000000 + self.get_mvv_lva(m, board);
        }

        for i in 0..move_list.len() {
            let mut best_pick_score = -2000000000;
            let mut best_pick_idx = i;
            for j in i..move_list.len() {
                if move_scores[j] > best_pick_score {
                    best_pick_score = move_scores[j];
                    best_pick_idx = j;
                }
            }
            {
                let moves_slice = move_list.as_mut_slice();
                moves_slice.swap(i, best_pick_idx);
                move_scores.swap(i, best_pick_idx);
            }

            let m = move_list.iter().nth(i).unwrap().clone();
            let undo = board.make_move(m);
            
            let us = if board.side_to_move == Color::White { Color::Black } else { Color::White };
            let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
            if board.is_square_attacked(king_sq, board.side_to_move) {
                board.unmake_move(m, undo);
                continue;
            }

            let score = -self.quiescence(board, -beta, -alpha);
            board.unmake_move(m, undo);

            if score >= beta { return beta; }
            if score > alpha { alpha = score; }
        }
        alpha
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
    if score > 30000 {
        score + ply
    } else if score < -30000 {
        score - ply
    } else {
        score
    }
}

fn score_from_tt(score: i32, ply: i32) -> i32 {
    if score > 30000 {
        score - ply
    } else if score < -30000 {
        score + ply
    } else {
        score
    }
}