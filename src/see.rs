use crate::board::Board;
use crate::moves::{self, Move};
use crate::types::{Color, PieceType, Square};
use crate::movegen;

pub fn see(board: &Board, m: Move) -> i32 {
    let from = moves::from_sq(m);
    let to = moves::to_sq(m);
    
    let mut gain = [0i32; 32];
    let mut d = 0;
    
    let mut from_set = 1u64 << from;
    let mut occ = board.occupancy[2];

    let mut side = board.side_to_move;
    
    let att_pt = board.piece_type_on(from).unwrap();
    let victim_pt = board.piece_type_on(to);
    
    gain[d] = if let Some(pt) = victim_pt {
        piece_value(pt)
    } else {
        0
    };
    
    let mut current_attacker_value = piece_value(att_pt);
    
    loop {
        d += 1;
        gain[d] = current_attacker_value - gain[d - 1];
        
        if std::cmp::max(-gain[d - 1], gain[d]) < 0 {
            break;
        }
        
        occ ^= from_set;
        
        side = if side == Color::White { Color::Black } else { Color::White };
        
        let mut next_pt = PieceType::Pawn;
        from_set = get_least_valuable_attacker(board, to, occ, side, &mut next_pt);
        
        if from_set == 0 {
            break;
        }
        
        current_attacker_value = piece_value(next_pt);
    }
    
    while d > 1 {
        d -= 1;
        gain[d - 1] = -std::cmp::max(-gain[d - 1], gain[d]);
    }
    
    gain[0]
}

fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 20000,
    }
}

fn get_least_valuable_attacker(
    board: &Board, 
    sq: Square, 
    occ: u64, 
    side: Color,
    piece_type: &mut PieceType
) -> u64 {
    let them = if side == Color::White { Color::Black } else { Color::White };
    
    let pawns = board.pieces[PieceType::Pawn as usize][side as usize] & occ;
    let pawn_attack_mask = movegen::pawn_attacks(them, sq);
    let attackers = pawns & pawn_attack_mask;
    if attackers != 0 {
        *piece_type = PieceType::Pawn;
        return attackers & attackers.wrapping_neg(); // LSB
    }

    let knights = board.pieces[PieceType::Knight as usize][side as usize] & occ;
    let attackers = knights & movegen::knight_attacks(sq);
    if attackers != 0 {
        *piece_type = PieceType::Knight;
        return attackers & attackers.wrapping_neg();
    }

    let bishops = board.pieces[PieceType::Bishop as usize][side as usize] & occ;
    let attackers = bishops & movegen::get_bishop_attacks(sq, occ);
    if attackers != 0 {
        *piece_type = PieceType::Bishop;
        return attackers & attackers.wrapping_neg();
    }

    let rooks = board.pieces[PieceType::Rook as usize][side as usize] & occ;
    let attackers = rooks & movegen::get_rook_attacks(sq, occ);
    if attackers != 0 {
        *piece_type = PieceType::Rook;
        return attackers & attackers.wrapping_neg();
    }

    let queens = board.pieces[PieceType::Queen as usize][side as usize] & occ;
    let attackers = queens & (movegen::get_bishop_attacks(sq, occ) | movegen::get_rook_attacks(sq, occ));
    if attackers != 0 {
        *piece_type = PieceType::Queen;
        return attackers & attackers.wrapping_neg();
    }

    let kings = board.pieces[PieceType::King as usize][side as usize] & occ;
    let attackers = kings & movegen::king_attacks(sq);
    if attackers != 0 {
        *piece_type = PieceType::King;
        return attackers & attackers.wrapping_neg();
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;
    use crate::moves;
    use crate::movegen;

    #[test]
    fn test_see_basic() {
        movegen::init();
        
        let board = Board::from_fen("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let m = moves::new(28, 35, moves::CAPTURE_FLAG); // e4xd5
        let see_val = see(&board, m);
        assert_eq!(see_val, 100, "Undefended pawn capture should be +100");
    }
    
    #[test]
    fn test_see_defended_piece() {
        movegen::init();
        
        let board = Board::from_fen("4k3/8/4p3/3p4/8/8/3Q4/4K3 w - - 0 1").unwrap();
        let m = moves::new(11, 35, moves::CAPTURE_FLAG); // Qd2xd5
        let see_val = see(&board, m);
        assert!(see_val < 0, "Queen taking defended pawn should be negative, got {}", see_val);
    }
    
    #[test]
    fn test_see_winning_exchange() {
        movegen::init();
        
        // RxN where knight is defended by pawn - should be positive (Knight 320 - Rook 500 + Pawn recaptures... wait)
        // Actually: White Rook takes Black Knight (320), Black pawn retakes (-500)
        // Net for white: 320 - 500 = -180, so this should be negative
        let board = Board::from_fen("4k3/8/3p4/4n3/8/8/4R3/4K3 w - - 0 1").unwrap();
        let m = moves::new(12, 36, moves::CAPTURE_FLAG); // Re2xe5
        let see_val = see(&board, m);
        assert!(see_val < 0, "RxN defended by pawn should be losing, got {}", see_val);
    }
    
    #[test]
    fn test_see_equal_exchange() {
        movegen::init();
        
        // Knight takes knight
        let board = Board::from_fen("4k3/8/8/4n3/8/8/4N3/4K3 w - - 0 1").unwrap();
        let m = moves::new(12, 36, moves::CAPTURE_FLAG); // Ne2xe5
        let see_val = see(&board, m);
        assert_eq!(see_val, 320, "NxN undefended should be +320, got {}", see_val);
    }
}