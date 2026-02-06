pub mod magic;

use crate::board::{Board, Color, PieceType, bitboard::*};
use lazy_static::lazy_static;

const KNIGHT_ATTACKS: [[i8; 2]; 8] = [
    [-2, -1], [-2, 1], [-1, -2], [-1, 2],
    [1, -2], [1, 2], [2, -1], [2, 1],
];

const KING_ATTACKS: [[i8; 2]; 8] = [
    [-1, -1], [-1, 0], [-1, 1], [0, -1],
    [0, 1], [1, -1], [1, 0], [1, 1],
];

lazy_static! {
    static ref KNIGHT_ATTACK_TABLE: [Bitboard; 64] = {
        let mut table = [0; 64];
        for sq in 0..64 {
            table[sq as usize] = generate_knight_attacks(sq);
        }
        table
    };
}

lazy_static! {
    static ref KING_ATTACK_TABLE: [Bitboard; 64] = {
        let mut table = [0; 64];
        for sq in 0..64 {
            table[sq as usize] = generate_king_attacks(sq);
        }
        table
    };
}

fn generate_knight_attacks(sq: u8) -> Bitboard {
    let mut attacks = 0;
    let rank = get_rank(sq);
    let file = get_file(sq);

    for i in 0..8 {
        let to_rank = rank as i8 + KNIGHT_ATTACKS[i][0];
        let to_file = file as i8 + KNIGHT_ATTACKS[i][1];

        if to_rank >= 0 && to_rank < 8 && to_file >= 0 && to_file < 8 {
            let to_sq = (to_rank * 8 + to_file) as u8;
            attacks |= square_mask(to_sq);
        }
    }
    attacks
}

fn generate_king_attacks(sq: u8) -> Bitboard {
    let mut attacks = 0;
    let rank = get_rank(sq);
    let file = get_file(sq);

    for i in 0..8 {
        let to_rank = rank as i8 + KING_ATTACKS[i][0];
        let to_file = file as i8 + KING_ATTACKS[i][1];

        if to_rank >= 0 && to_rank < 8 && to_file >= 0 && to_file < 8 {
            let to_sq = (to_rank * 8 + to_file) as u8;
            attacks |= square_mask(to_sq);
        }
    }
    attacks
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub promotion: Option<PieceType>,
}

pub fn get_knight_attacks(sq: u8) -> Bitboard {
    KNIGHT_ATTACK_TABLE[sq as usize]
}

pub fn get_rook_attacks(sq: u8, blockers: Bitboard) -> Bitboard {
    let magic = &magic::ROOK_MAGICS[sq as usize];
    let index = (blockers & magic.mask).wrapping_mul(magic.magic) >> magic.shift;
    magic.attacks[index as usize]
}

pub fn get_bishop_attacks(sq: u8, blockers: Bitboard) -> Bitboard {
    let magic = &magic::BISHOP_MAGICS[sq as usize];
    let index = (blockers & magic.mask).wrapping_mul(magic.magic) >> magic.shift;
    magic.attacks[index as usize]
}

pub fn generate_moves(board: &Board) -> Vec<Move> {
    let mut moves = Vec::new();
    let side_to_move = board.get_side_to_move();
    let occupied = board.get_occupied();
    let friendly_pieces = board.get_color_bb(side_to_move);

    let (us, them) = (side_to_move, side_to_move.opposite());
    let our_pieces = board.get_color_bb(us);
    let their_pieces = board.get_color_bb(them);
    let all_pieces = our_pieces | their_pieces;

    // Pawn moves
    let pawns = board.get_piece_bb(PieceType::Pawn) & our_pieces;
    for from_sq in iter_bits(pawns) {
        let rank = get_rank(from_sq);
        let single_push_sq = if us == Color::White { from_sq + 8 } else { from_sq - 8 };

        // Single push
        if !has_bit(all_pieces, single_push_sq) {
            if (us == Color::White && rank == 6) || (us == Color::Black && rank == 1) {
                moves.push(Move { from: from_sq, to: single_push_sq, promotion: Some(PieceType::Queen) });
                moves.push(Move { from: from_sq, to: single_push_sq, promotion: Some(PieceType::Rook) });
                moves.push(Move { from: from_sq, to: single_push_sq, promotion: Some(PieceType::Bishop) });
                moves.push(Move { from: from_sq, to: single_push_sq, promotion: Some(PieceType::Knight) });
            } else {
                moves.push(Move { from: from_sq, to: single_push_sq, promotion: None });
            }

            // Double push
            let can_double_push = if us == Color::White { rank == 1 } else { rank == 6 };
            if can_double_push {
                let double_push_sq = if us == Color::White { from_sq + 16 } else { from_sq - 16 };
                if !has_bit(all_pieces, double_push_sq) {
                    moves.push(Move { from: from_sq, to: double_push_sq, promotion: None });
                }
            }
        }

        // Captures
        let attacks = if us == Color::White {
            ((square_mask(from_sq) & !FILE_A) << 7) | ((square_mask(from_sq) & !FILE_H) << 9)
        } else {
            ((square_mask(from_sq) & !FILE_H) >> 7) | ((square_mask(from_sq) & !FILE_A) >> 9)
        };
        let valid_captures = attacks & their_pieces;
        for to_sq in iter_bits(valid_captures) {
             if (us == Color::White && rank == 6) || (us == Color::Black && rank == 1) {
                moves.push(Move { from: from_sq, to: to_sq, promotion: Some(PieceType::Queen) });
                moves.push(Move { from: from_sq, to: to_sq, promotion: Some(PieceType::Rook) });
                moves.push(Move { from: from_sq, to: to_sq, promotion: Some(PieceType::Bishop) });
                moves.push(Move { from: from_sq, to: to_sq, promotion: Some(PieceType::Knight) });
            } else {
                moves.push(Move { from: from_sq, to: to_sq, promotion: None });
            }
        }

        // En passant
        if let Some(ep_sq) = board.get_en_passant_square() {
            if attacks & square_mask(ep_sq) != 0 {
                moves.push(Move { from: from_sq, to: ep_sq, promotion: None });
            }
        }
    }

    // Knight moves
    let knights = board.get_piece_bb(PieceType::Knight) & friendly_pieces;
    for from_sq in iter_bits(knights) {
        let attacks = KNIGHT_ATTACK_TABLE[from_sq as usize] & !friendly_pieces;
        for to_sq in iter_bits(attacks) {
            moves.push(Move { from: from_sq, to: to_sq, promotion: None });
        }
    }

    // Bishop moves
    let bishops = board.get_piece_bb(PieceType::Bishop) & friendly_pieces;
    for from_sq in iter_bits(bishops) {
        let attacks = get_bishop_attacks(from_sq, occupied) & !friendly_pieces;
        for to_sq in iter_bits(attacks) {
            moves.push(Move { from: from_sq, to: to_sq, promotion: None });
        }
    }

    // Rook moves
    let rooks = board.get_piece_bb(PieceType::Rook) & friendly_pieces;
    for from_sq in iter_bits(rooks) {
        let attacks = get_rook_attacks(from_sq, occupied) & !friendly_pieces;
        for to_sq in iter_bits(attacks) {
            moves.push(Move { from: from_sq, to: to_sq, promotion: None });
        }
    }

    // Queen moves
    let queens = board.get_piece_bb(PieceType::Queen) & friendly_pieces;
    for from_sq in iter_bits(queens) {
        let attacks = (get_rook_attacks(from_sq, occupied) | get_bishop_attacks(from_sq, occupied)) & !friendly_pieces;
        for to_sq in iter_bits(attacks) {
            moves.push(Move { from: from_sq, to: to_sq, promotion: None });
        }
    }

    // King moves
    let king_sq_bb = board.get_piece_bb(PieceType::King) & our_pieces;
    if king_sq_bb != 0 {
        let king_sq = king_sq_bb.trailing_zeros() as u8;
        let attacks = KING_ATTACK_TABLE[king_sq as usize] & !friendly_pieces;
        for to_sq in iter_bits(attacks) {
            moves.push(Move { from: king_sq, to: to_sq, promotion: None });
        }
    }


    // Castling
    let castling_rights = board.get_castling_rights();
    if us == Color::White {
        if castling_rights.white_kingside {
            if !has_bit(all_pieces, 5) && !has_bit(all_pieces, 6) {
                if !is_square_attacked(board, 4, them) && !is_square_attacked(board, 5, them) && !is_square_attacked(board, 6, them) {
                    moves.push(Move { from: 4, to: 6, promotion: None });
                }
            }
        }
        if castling_rights.white_queenside {
            if !has_bit(all_pieces, 1) && !has_bit(all_pieces, 2) && !has_bit(all_pieces, 3) {
                if !is_square_attacked(board, 2, them) && !is_square_attacked(board, 3, them) && !is_square_attacked(board, 4, them) {
                    moves.push(Move { from: 4, to: 2, promotion: None });
                }
            }
        }
    } else {
        if castling_rights.black_kingside {
            if !has_bit(all_pieces, 61) && !has_bit(all_pieces, 62) {
                if !is_square_attacked(board, 60, them) && !is_square_attacked(board, 61, them) && !is_square_attacked(board, 62, them) {
                    moves.push(Move { from: 60, to: 62, promotion: None });
                }
            }
        }
        if castling_rights.black_queenside {
            if !has_bit(all_pieces, 57) && !has_bit(all_pieces, 58) && !has_bit(all_pieces, 59) {
                if !is_square_attacked(board, 58, them) && !is_square_attacked(board, 59, them) && !is_square_attacked(board, 60, them) {
                    moves.push(Move { from: 60, to: 58, promotion: None });
                }
            }
        }
    }

    moves
}

pub fn is_square_attacked(board: &Board, sq: u8, by_color: Color) -> bool {
    let pawns = board.get_piece_bb(PieceType::Pawn) & board.get_color_bb(by_color);
    if by_color == Color::White {
        if ((pawns & !FILE_A) << 7) & square_mask(sq) != 0 { return true; }
        if ((pawns & !FILE_H) << 9) & square_mask(sq) != 0 { return true; }
    } else {
        if ((pawns & !FILE_H) >> 7) & square_mask(sq) != 0 { return true; }
        if ((pawns & !FILE_A) >> 9) & square_mask(sq) != 0 { return true; }
    }

    let knights = board.get_piece_bb(PieceType::Knight) & board.get_color_bb(by_color);
    if KNIGHT_ATTACK_TABLE[sq as usize] & knights != 0 { return true; }

    let bishops = board.get_piece_bb(PieceType::Bishop) & board.get_color_bb(by_color);
    if get_bishop_attacks(sq, board.get_occupied()) & bishops != 0 { return true; }

    let rooks = board.get_piece_bb(PieceType::Rook) & board.get_color_bb(by_color);
    if get_rook_attacks(sq, board.get_occupied()) & rooks != 0 { return true; }

    let queens = board.get_piece_bb(PieceType::Queen) & board.get_color_bb(by_color);
    if (get_rook_attacks(sq, board.get_occupied()) | get_bishop_attacks(sq, board.get_occupied())) & queens != 0 { return true; }

    let kings = board.get_piece_bb(PieceType::King) & board.get_color_bb(by_color);
    if KING_ATTACK_TABLE[sq as usize] & kings != 0 { return true; }

    false
}
