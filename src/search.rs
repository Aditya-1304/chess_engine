use crate::{
    board::Board, book::OpeningBook, eval, movegen, moves::{self, Move, MoveList}, see, syzygy, tt::{TTFlag, TranspositionTable}, types::{Color, PieceType}
};
use std::time::Instant;

const INF: i32 = 32000;
pub const MATE_SCORE: i32 = 31000;

pub struct Searcher {
    pub nodes: u64,
    pub start_time: Instant,
    pub time_soft_limit: u128,
    pub time_hard_limit: u128,
    pub stop: bool,
    pub tt: TranspositionTable,
    pub book: OpeningBook,
    pub killers: [[Option<Move>; 2]; 64],
    pub history: [[[i32; 64]; 2]; 6],
    pub counter_moves: [[Option<Move>; 64]; 6],
    pub prev_move: Option<Move>,
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
        println!("White Pawn A1 (Index 0):   {:016x}", keys.pieces[0][0][0]);
        println!("Black King H8 (Index 767): {:016x}", keys.pieces[5][1][63]);
        println!("Castle WK (Index 768):     {:016x}", keys.castling[1]);
        println!("--------------------------");

        Self {
            nodes: 0,
            start_time: Instant::now(),
            time_soft_limit: 0,
            time_hard_limit: 0,
            stop: false,
            tt: TranspositionTable::new(64), // 64MB default
            book,
            killers: [[None; 2]; 64],
            history: [[[0; 64]; 2]; 6],
            counter_moves: [[None; 64]; 6],
            prev_move: None,
        }
    }

    pub fn search(&mut self, board: &mut Board, depth: u8) -> (i32, Option<Move>) {
        self.nodes = 0;
        self.start_time = Instant::now();
        self.stop = false;
        self.tt.new_search();
        self.killers = [[None; 2]; 64];
        self.age_history();

        if self.time_soft_limit == 0 {
            self.time_soft_limit = u128::MAX;
        }
        if self.time_hard_limit == 0 {
            self.time_hard_limit = u128::MAX;
        }

        let mut best_move = None;
        let mut score = 0;

        let mut root_moves = MoveList::new();
        board.generate_pseudo_legal_moves(&mut root_moves);
        let mut legal_moves = Vec::new();
        for &m in root_moves.iter() {
            let undo = board.make_move(m);
            let us = if board.side_to_move == Color::White {
                Color::Black
            } else {
                Color::White
            };
            let king_sq =
                board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
            if !board.is_square_attacked(king_sq, board.side_to_move) {
                legal_moves.push(m);
            }
            board.unmake_move(m, undo);
        }

        if legal_moves.len() == 1 {
            return (0, Some(legal_moves[0]));
        }

        let mut prev_best_move = None;
        let mut stability = 0;
        let mut last_iter_time = 0_u128;

        println!("info string Zobrist Hash: {:x}", board.zobrist_hash);

        // Book Probing
        if let Some(book_move) = self.book.get_move(board.zobrist_hash) {
            let mut move_list = MoveList::new();
            board.generate_pseudo_legal_moves(&mut move_list);
            let mut found_move = None;

            for &m in move_list.iter() {
                if moves::from_sq(m) == moves::from_sq(book_move)
                    && moves::to_sq(m) == moves::to_sq(book_move)
                {
                    if moves::is_promotion(book_move) {
                        if moves::is_promotion(m)
                            && moves::promotion_piece(m) == moves::promotion_piece(book_move)
                        {
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
            }
        }

        // Syzygy DTZ Root Probing
       if board.occupancy[2].count_ones() <= 6 {
            if let Some(tb) = crate::syzygy::get_global_syzygy() {
                if board.occupancy[2].count_ones() <= tb.max_pieces() {
                    if let Some((from, to, promo, wdl)) = syzygy::probe_root(board, &tb) {
                        let mut move_list = MoveList::new();
                        board.generate_pseudo_legal_moves(&mut move_list);

                        for &m in move_list.iter() {
                            if moves::from_sq(m) == from && moves::to_sq(m) == to {
                                let promo_match = if promo > 0 {
                                    if moves::is_promotion(m) {
                                        let our_promo = match moves::promotion_piece(m) {
                                            PieceType::Knight => 1,
                                            PieceType::Bishop => 2,
                                            PieceType::Rook => 3,
                                            PieceType::Queen => 4,
                                            _ => 0,
                                        };
                                        our_promo == promo
                                    } else {
                                        false
                                    }
                                } else {
                                    !moves::is_promotion(m)
                                };

                                if promo_match {
                                    let undo = board.make_move(m);
                                    let us = if board.side_to_move == Color::White {
                                        Color::Black
                                    } else {
                                        Color::White
                                    };
                                    let king_sq = board.pieces[PieceType::King as usize][us as usize].trailing_zeros() as u8;
                                    let legal = !board.is_square_attacked(king_sq, board.side_to_move);
                                    board.unmake_move(m, undo);

                                    if legal {
                                        let tb_score = match wdl {
                                            1 => 29000,   
                                            -1 => -29000, 
                                            _ => 0,       
                                        };
                                        println!("info string TB root move: {} (wdl={})", moves::format(m), wdl);
                                        return (tb_score, Some(m));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut alpha = -INF;
        let mut beta = INF;

        // Iterative Deepening with Aspiration Windows
        for d in 1..=depth {
            let elapsed = self.start_time.elapsed().as_millis();
            if self.time_hard_limit != u128::MAX && elapsed >= self.time_hard_limit {
                self.stop = true;
            }
            if self.stop {
                break;
            }

            if d > 1 && self.time_soft_limit != u128::MAX {
                let projected = elapsed + last_iter_time.saturating_mul(3) / 2 + 5;
                if projected >= self.time_soft_limit {
                    break;
                }
            }

            let iter_start_time = self.start_time.elapsed().as_millis();

            // Aspiration Windows
            let mut delta = 50;
            if d > 4 {
                alpha = (-INF).max(score - delta);
                beta = (INF).min(score + delta);
            } else {
                alpha = -INF;
                beta = INF;
            }

            let mut search_score;
            loop {
                let (s, m) = self.negamax(board, d, 0, alpha, beta, true);
                search_score = s;

                if self.stop {
                    break;
                }

                if s <= alpha {
                    // Fail low: widen alpha downwards
                    alpha = (-INF).max(alpha - delta);
                    delta += delta / 2;
                } else if s >= beta {
                    // Fail high: widen beta upwards
                    if let Some(mv) = m {
                        best_move = Some(mv);
                    }
                    beta = (INF).min(beta + delta);
                    delta += delta / 2;
                } else {
                    if let Some(mv) = m {
                        best_move = Some(mv);
                    }
                    break;
                }

                // Safety valve
                if delta > 3000 {
                    alpha = -INF;
                    beta = INF;
                }
            }

            if self.stop {
                break;
            }

            score = search_score;
            if let Some(mv) = best_move {
                if Some(mv) == prev_best_move {
                    stability += 1;
                } else {
                    stability = 0;
                }
                prev_best_move = Some(mv);
            }

            let time_elapsed = self.start_time.elapsed().as_millis();
            last_iter_time = time_elapsed.saturating_sub(iter_start_time);
            let nps = if time_elapsed > 0 {
                (self.nodes as u128 * 1000) / time_elapsed
            } else {
                0
            };

            print!("info depth {} score ", d);
            if score > 30000 {
                let mate_in = (31000 - score + 1) / 2;
                print!("mate {}", mate_in);
            } else if score < -30000 {
                let mate_in = (31000 + score) / 2;
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
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            println!(" nodes {} nps {} time {}", self.nodes, nps, time_elapsed);

            if self.time_hard_limit != 0 && time_elapsed >= self.time_hard_limit {
                self.stop = true;
                break;
            }
            if self.time_soft_limit != 0 && time_elapsed >= self.time_soft_limit {
                self.stop = true;
                break;
            }

            if stability >= 4 && time_elapsed > self.time_soft_limit / 2 {
                self.stop = true;
                break;
            }
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
            if self.time_hard_limit != u128::MAX
                && self.start_time.elapsed().as_millis() >= self.time_hard_limit
            {
                self.stop = true;
            }
        }
        if self.stop {
            return (0, None);
        }

        let is_root = ply == 0;
        if !is_root && (board.halfmove_clock >= 100 || board.is_repetition()) {
            return (0, None);
        }

        // Syzygy Probing
        if !is_root && board.occupancy[2].count_ones() <= 6 {
            if let Some(tb) = syzygy::get_global_syzygy() {
                if board.occupancy[2].count_ones() <= tb.max_pieces() {
                    if let Some(wdl) = syzygy::probe_wdl(board, &tb) {
                        let tb_score = match wdl {
                            pyrrhic_rs::WdlProbeResult::Win => 30000 - ply,
                            pyrrhic_rs::WdlProbeResult::Loss => -30000 + ply,
                            _ => 0,
                        };

                        match wdl {
                            pyrrhic_rs::WdlProbeResult::Win => {
                                if tb_score >= beta {
                                    return (tb_score, None);
                                }
                            }

                            pyrrhic_rs::WdlProbeResult::Loss => {
                                if tb_score <= alpha {
                                    return (tb_score, None);
                                }
                            }

                            _ => {
                                if tb_score >= beta || tb_score <= alpha {
                                    return (tb_score, None);
                                }
                            }
                        }
                    }
                }
            }
        }

        let in_check = board.is_square_attacked(
            board.pieces[PieceType::King as usize][board.side_to_move as usize].trailing_zeros()
                as u8,
            if board.side_to_move == Color::White {
                Color::Black
            } else {
                Color::White
            },
        );

        if in_check {
            depth += 1;
        }

        if depth == 0 {
            return (self.quiescence(board, alpha, beta), None);
        }

        self.nodes += 1;


        let mut tt_move = None;
        if let Some((mv, sc, d, flag)) = self.tt.probe(board.zobrist_hash) {
            let is_valid = if mv != 0 {
                let from = moves::from_sq(mv);
                let to = moves::to_sq(mv);
                if from == to {
                    false
                } else {
                    let pt = board.piece_type_on(from);
                    if let Some(p) = pt {
                        (board.pieces[p as usize][board.side_to_move as usize] & (1 << from)) != 0
                    } else {
                        false
                    }
                }
            } else {
                true 
            };

            if is_valid {
                tt_move = if mv != 0 { Some(mv) } else { None };
                if !is_root && d >= depth {
                    let tt_score = score_from_tt(sc, ply);
                    match flag {
                        TTFlag::Exact => return (tt_score, tt_move),
                        TTFlag::Beta => {
                            if tt_score >= beta {
                                return (tt_score, tt_move);
                            }
                        }
                        TTFlag::Alpha => {
                            if tt_score <= alpha {
                                return (tt_score, tt_move);
                            }
                        }
                    }
                }
            }
        }

        // Adaptive Null Move Pruning
        if do_null && !in_check && !is_root && depth >= 3 {
            // Check we have non-pawn material
            let dominated_by_pawns = 
                (board.pieces[PieceType::Knight as usize][board.side_to_move as usize] |
                 board.pieces[PieceType::Bishop as usize][board.side_to_move as usize] |
                 board.pieces[PieceType::Rook as usize][board.side_to_move as usize] |
                 board.pieces[PieceType::Queen as usize][board.side_to_move as usize]) == 0;
            
            if !dominated_by_pawns {
                let static_eval = eval::evaluate(board);
                if static_eval >= beta {
                    let r = if depth > 6 { 3 } else { 2 };
                    let old_ep = board.make_null_move();
                    let (score, _) =
                        self.negamax(board, depth - 1 - r, ply + 1, -beta, -beta + 1, false);
                    board.unmake_null_move(old_ep);
                    let null_score = -score;
                    // Don't trust mate scores from null move
                    if null_score >= beta && null_score < 30000 {
                        return (beta, None);
                    }
                }
            }
        }

        if !is_root && !in_check&& depth <= 6 {
            let static_eval = eval::evaluate(board);
            let margin = 80 * (depth as i32);
            if static_eval - margin >= beta {
                return (static_eval - margin, None);
            }
        }

        if tt_move.is_none() && depth >= 4 {
            let iid_depth = depth - 2;
            let (_, iid_move) = self.negamax(board, iid_depth, ply, alpha, beta, false);
            if let Some(m) = iid_move {
                tt_move = Some(m);
            }
        }

        

        let mut move_list = MoveList::new();
        board.generate_pseudo_legal_moves(&mut move_list);

        // Score moves ONCE
        let mut move_scores = [0; 256];
        let move_slice = move_list.as_mut_slice();
        for i in 0..move_slice.len() {
            let m = move_slice[i];
            if Some(m) == tt_move {
                move_scores[i] = 2000000000;
            } else if moves::is_capture(m) {
                move_scores[i] = 1000000 + self.get_mvv_lva(m, board);
            } else {
                if ply < 64 {
                    if self.killers[ply as usize][0] == Some(m) {
                        move_scores[i] = 900000;
                    } else if self.killers[ply as usize][1] == Some(m) {
                        move_scores[i] = 800000;
                    }
                }

                if let Some(prev) = self.prev_move{
                    let prev_pt = board.piece_type_on(moves::to_sq(prev));
                    if let Some(ppt) = prev_pt {
                        let prev_to = moves::to_sq(prev);
                        if self.counter_moves[ppt as usize][prev_to as usize] == Some(m) {
                            move_scores[i] = 700000;
                        }
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

        

        // Futility Pruning Setup
        let mut futility_pruning = false;
        if !is_root && !in_check && depth <= 3 && alpha < beta - 1 {
            let eval = eval::evaluate(board);
            let margin = 150 * (depth as i32);
            if eval + margin <= alpha {
                futility_pruning = true;
            }
        }

        let mut best_score = -INF;
        let mut best_move = None;
        let mut legal_moves = 0;
        let mut skipped_moves = 0;
        let alpha_orig = alpha;
        let mut searched_quiets: [Move; 64] = [0; 64];
        let mut quiet_count = 0;

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
            {
                let moves_slice = move_list.as_mut_slice();
                moves_slice.swap(i, best_pick_idx);
                move_scores.swap(i, best_pick_idx);
            }

            let m = move_list.iter().nth(i).unwrap().clone();

            // Futility Pruning Check
            if futility_pruning && !moves::is_capture(m) && !moves::is_promotion(m) {
                skipped_moves += 1;
                continue;
            }

            let lmp_threshold = if depth <= 1 { 3 } else if depth <= 2 { 6 } else if depth <= 3 { 10 } else { 15 };
            if !is_root && !in_check && depth <= 4 && legal_moves > lmp_threshold 
                && !moves::is_capture(m) && !moves::is_promotion(m) {
                continue;
            }


            if !is_root && depth >= 1 && moves::is_capture(m) && legal_moves > 0 {
                let see_value = see::see(board, m);

                let threshold = -20 * (depth as i32);
                if see_value < threshold {
                    continue;
                } 
            }

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

            let mut score;
            let old_prev = self.prev_move;
            self.prev_move = Some(m);
            if legal_moves == 1 {
                let (s, _) = self.negamax(board, depth - 1, ply + 1, -beta, -alpha, true);
                score = -s;
            } else {
                let mut reduction = 0;
                if depth >= 3
                    && legal_moves > 1
                    && !moves::is_capture(m)
                    && !moves::is_promotion(m)
                    && !in_check
                {
                    let lmr_depth = (depth as f64).ln();
                    let lmr_move = (legal_moves as f64).ln();
                    reduction = (1.0 + lmr_depth * lmr_move / 2.0) as u8;
                    if reduction >= depth {
                        reduction = depth - 1;
                    }
                }

                let (s, _) = self.negamax(
                    board,
                    depth - 1 - reduction,
                    ply + 1,
                    -alpha - 1,
                    -alpha,
                    true,
                );
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

            self.prev_move = old_prev;
            board.unmake_move(m, undo);
            if self.stop {
                return (0, None);
            }

            if !moves::is_capture(m) && quiet_count < 64 {
                searched_quiets[quiet_count] = m;
                quiet_count += 1;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(m);
                if score > alpha {
                    alpha = score;
                    if !moves::is_capture(m) {
                        let pt = board.piece_type_on(moves::from_sq(m)).unwrap();
                        let c = board.side_to_move;
                        let to = moves::to_sq(m);
                        self.history[pt as usize][c as usize][to as usize] +=
                            (depth as i32) * (depth as i32);
                        if self.history[pt as usize][c as usize][to as usize] > 20000 {
                            self.history[pt as usize][c as usize][to as usize] /= 2;
                        }
                        if ply < 64 && self.killers[ply as usize][0] != Some(m) {
                            self.killers[ply as usize][1] = self.killers[ply as usize][0];
                            self.killers[ply as usize][0] = Some(m);
                        }
                    }
                }
            }
            if alpha >= beta {
                if !moves::is_capture(m) {
                    let pt = board.piece_type_on(moves::from_sq(m)).unwrap();
                    let c = board.side_to_move;
                    let to = moves::to_sq(m);
                    self.history[pt as usize][c as usize][to as usize] +=
                        (depth as i32) * (depth as i32);

                    if self.history[pt as usize][c as usize][to as usize] > 20000 {
                        self.history[pt as usize][c as usize][to as usize] /= 2;
                    }

                    for j in 0..quiet_count.saturating_sub(1){
                        let failed_m = searched_quiets[j];
                        if let Some(pt) = board.piece_type_on(moves::from_sq(failed_m)) {
                            let to = moves::to_sq(failed_m);
                            self.history[pt as usize][board.side_to_move as usize][to as usize] -= (depth as i32) * (depth as i32);
                            if self.history[pt as usize][board.side_to_move as usize][to as usize] < -20000 {
                                self.history[pt as usize][board.side_to_move as usize][to as usize] = -20000;
                            }
                        }
                    }

                    if let Some(prev) = old_prev {
                        if let Some(prev_pt) = board.piece_type_on(moves::to_sq(prev)) {
                            let prev_to = moves::to_sq(prev);
                            self.counter_moves[prev_pt as usize][prev_to as usize] = Some(m);
                        }
                    }

                    if ply < 64 && self.killers[ply as usize][0] != Some(m) {
                        self.killers[ply as usize][1] = self.killers[ply as usize][0];
                        self.killers[ply as usize][0] = Some(m);
                    }
                }
                break;
            }
        }

        if legal_moves == 0 {
            if in_check {
                return (-MATE_SCORE + ply, None);
            } else if skipped_moves > 0 {
                return (alpha, None);
            } else {
                return (0, None);
            }
        }

        let flag = if best_score <= alpha_orig {
            TTFlag::Alpha
        } else if best_score >= beta {
            TTFlag::Beta
        } else {
            TTFlag::Exact
        };

        let move_to_store = if flag == TTFlag::Alpha {
            None 
        } else {
            best_move
        };

        self.tt.store(
            board.zobrist_hash,
            move_to_store,
            score_to_tt(best_score, ply),
            depth,
            flag,
        );
        (best_score, best_move)
    }

    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32) -> i32 {
        if self.nodes & 2047 == 0 {
            if self.time_hard_limit != u128::MAX
                && self.start_time.elapsed().as_millis() >= self.time_hard_limit
            {
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

        // Delta Pruning
        let delta = 975; // Queen value + margin
        if stand_pat + delta < alpha {
            return alpha;
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let mut move_list = MoveList::new();
        movegen::generate_captures(board, &mut move_list);

        // Simple sort for qsearch
        let mut move_scores = [0; 256];
        for i in 0..move_list.len() {
            let m = move_list.get(i);
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

            let m = move_list.get(i);

            if see::see(board, m) < -50 {
                continue;
            }
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

    fn age_history(&mut self) {
        for pt in 0..6 {
            for c in 0..2 {
                for sq in 0..64 {
                    self.history[pt][c][sq] /= 2;
                }
            }
        }
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
