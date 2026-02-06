//! Position evaluation module.
//!
//! Evaluates chess positions using material counting and positional factors.
//! All scores are in centipawns from the perspective of the side to move.
//! Uses tapered evaluation (Fruit/CPW approach) to interpolate between
//! middlegame and endgame scores based on remaining material.

use std::ops::{Add, AddAssign, Sub, Neg};
use crate::board::{Board, Color, PieceType, bitboard::*};
use crate::movegen::{get_bishop_attacks, get_knight_attacks, get_rook_attacks};

// ---------------------------------------------------------------------------
// Public material constants (unchanged API)
// ---------------------------------------------------------------------------

/// Material values in centipawns
pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 330;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 900;

// ---------------------------------------------------------------------------
// Score struct – holds middlegame and endgame values
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Score {
    mg: i32,
    eg: i32,
}

impl Score {
    const ZERO: Score = Score { mg: 0, eg: 0 };

    const fn new(mg: i32, eg: i32) -> Self {
        Score { mg, eg }
    }
}

impl Add for Score {
    type Output = Score;
    fn add(self, rhs: Score) -> Score {
        Score { mg: self.mg + rhs.mg, eg: self.eg + rhs.eg }
    }
}

impl AddAssign for Score {
    fn add_assign(&mut self, rhs: Score) {
        self.mg += rhs.mg;
        self.eg += rhs.eg;
    }
}

impl Sub for Score {
    type Output = Score;
    fn sub(self, rhs: Score) -> Score {
        Score { mg: self.mg - rhs.mg, eg: self.eg - rhs.eg }
    }
}

impl Neg for Score {
    type Output = Score;
    fn neg(self) -> Score {
        Score { mg: -self.mg, eg: -self.eg }
    }
}

// ---------------------------------------------------------------------------
// Game phase
// ---------------------------------------------------------------------------

const PHASE_KNIGHT: i32 = 1;
const PHASE_BISHOP: i32 = 1;
const PHASE_ROOK: i32 = 2;
const PHASE_QUEEN: i32 = 4;
const MAX_PHASE: i32 = 24; // 4*1 + 4*1 + 4*2 + 2*4

fn compute_phase(board: &Board) -> i32 {
    let knights = popcount(board.get_piece_bb(PieceType::Knight)) as i32;
    let bishops = popcount(board.get_piece_bb(PieceType::Bishop)) as i32;
    let rooks = popcount(board.get_piece_bb(PieceType::Rook)) as i32;
    let queens = popcount(board.get_piece_bb(PieceType::Queen)) as i32;

    let phase = knights * PHASE_KNIGHT
        + bishops * PHASE_BISHOP
        + rooks * PHASE_ROOK
        + queens * PHASE_QUEEN;

    // Clamp to MAX_PHASE (shouldn't exceed, but be safe)
    phase.min(MAX_PHASE)
}

// ---------------------------------------------------------------------------
// Material values (MG / EG pairs)
// ---------------------------------------------------------------------------

const PAWN_SCORE: Score = Score::new(100, 120);
const KNIGHT_SCORE: Score = Score::new(320, 310);
const BISHOP_SCORE: Score = Score::new(330, 340);
const ROOK_SCORE: Score = Score::new(500, 520);
const QUEEN_SCORE: Score = Score::new(900, 900);
const BISHOP_PAIR: Score = Score::new(30, 50);

// ---------------------------------------------------------------------------
// Pawn structure (MG / EG pairs)
// ---------------------------------------------------------------------------

const DOUBLED_PAWN: Score = Score::new(-15, -25);
const ISOLATED_PAWN: Score = Score::new(-12, -20);

const PASSED_PAWN_MG: [i32; 8] = [0, 5, 10, 20, 35, 60, 90, 0];
const PASSED_PAWN_EG: [i32; 8] = [0, 15, 30, 50, 85, 140, 210, 0];

// ---------------------------------------------------------------------------
// Mobility weights (MG / EG)
// ---------------------------------------------------------------------------

const KNIGHT_MOBILITY: Score = Score::new(4, 4);
const BISHOP_MOBILITY: Score = Score::new(5, 5);
const ROOK_MOBILITY: Score = Score::new(2, 3);
const QUEEN_MOBILITY: Score = Score::new(1, 2);

// ---------------------------------------------------------------------------
// Piece-square tables – MG
// ---------------------------------------------------------------------------

const PAWN_PST_MG: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10,-20,-20, 10, 10,  5,
     5, -5,-10,  0,  0,-10, -5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5,  5, 10, 25, 25, 10,  5,  5,
    10, 10, 20, 30, 30, 20, 10, 10,
    50, 50, 50, 50, 50, 50, 50, 50,
     0,  0,  0,  0,  0,  0,  0,  0,
];

const PAWN_PST_EG: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    10, 10, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10,
    15, 15, 15, 20, 20, 15, 15, 15,
    25, 25, 25, 30, 30, 25, 25, 25,
    35, 35, 35, 40, 40, 35, 35, 35,
    60, 60, 60, 60, 60, 60, 60, 60,
     0,  0,  0,  0,  0,  0,  0,  0,
];

const KNIGHT_PST_MG: [i32; 64] = [
   -50,-40,-30,-30,-30,-30,-40,-50,
   -40,-20,  0,  5,  5,  0,-20,-40,
   -30,  5, 10, 15, 15, 10,  5,-30,
   -30,  0, 15, 20, 20, 15,  0,-30,
   -30,  5, 15, 20, 20, 15,  5,-30,
   -30,  0, 10, 15, 15, 10,  0,-30,
   -40,-20,  0,  0,  0,  0,-20,-40,
   -50,-40,-30,-30,-30,-30,-40,-50,
];

const KNIGHT_PST_EG: [i32; 64] = [
   -40,-30,-20,-20,-20,-20,-30,-40,
   -30,-15,  0,  5,  5,  0,-15,-30,
   -20,  5, 10, 15, 15, 10,  5,-20,
   -20,  0, 15, 20, 20, 15,  0,-20,
   -20,  5, 15, 20, 20, 15,  5,-20,
   -20,  0, 10, 15, 15, 10,  0,-20,
   -30,-15,  0,  0,  0,  0,-15,-30,
   -40,-30,-20,-20,-20,-20,-30,-40,
];

const BISHOP_PST_MG: [i32; 64] = [
   -20,-10,-10,-10,-10,-10,-10,-20,
   -10,  5,  0,  0,  0,  0,  5,-10,
   -10, 10, 10, 10, 10, 10, 10,-10,
   -10,  0, 10, 10, 10, 10,  0,-10,
   -10,  5,  5, 10, 10,  5,  5,-10,
   -10,  0,  5, 10, 10,  5,  0,-10,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -20,-10,-10,-10,-10,-10,-10,-20,
];

const BISHOP_PST_EG: [i32; 64] = [
   -20,-10,-10,-10,-10,-10,-10,-20,
   -10,  5,  0,  0,  0,  0,  5,-10,
   -10, 10, 10, 10, 10, 10, 10,-10,
   -10,  0, 10, 10, 10, 10,  0,-10,
   -10,  5,  5, 10, 10,  5,  5,-10,
   -10,  0,  5, 10, 10,  5,  0,-10,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -20,-10,-10,-10,-10,-10,-10,-20,
];

const ROOK_PST_MG: [i32; 64] = [
     0,  0,  0,  5,  5,  0,  0,  0,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     5, 10, 10, 10, 10, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];

const ROOK_PST_EG: [i32; 64] = [
     0,  0,  0,  5,  5,  0,  0,  0,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     5, 10, 10, 10, 10, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];

const QUEEN_PST_MG: [i32; 64] = [
   -20,-10,-10, -5, -5,-10,-10,-20,
   -10,  0,  5,  0,  0,  0,  0,-10,
   -10,  5,  5,  5,  5,  5,  0,-10,
     0,  0,  5,  5,  5,  5,  0, -5,
    -5,  0,  5,  5,  5,  5,  0, -5,
   -10,  0,  5,  5,  5,  5,  0,-10,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -20,-10,-10, -5, -5,-10,-10,-20,
];

const QUEEN_PST_EG: [i32; 64] = [
   -20,-10,-10, -5, -5,-10,-10,-20,
   -10,  0,  5,  0,  0,  0,  0,-10,
   -10,  5,  5,  5,  5,  5,  0,-10,
     0,  0,  5,  5,  5,  5,  0, -5,
    -5,  0,  5,  5,  5,  5,  0, -5,
   -10,  0,  5,  5,  5,  5,  0,-10,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -20,-10,-10, -5, -5,-10,-10,-20,
];

const KING_PST_MG: [i32; 64] = [
    20, 30, 10,  0,  0, 10, 30, 20,
    20, 20,  0,  0,  0,  0, 20, 20,
   -10,-20,-20,-20,-20,-20,-20,-10,
   -20,-30,-30,-40,-40,-30,-30,-20,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
];

const KING_PST_EG: [i32; 64] = [
   -50,-30,-20,-20,-20,-20,-30,-50,
   -30,-10,  0,  5,  5,  0,-10,-30,
   -20,  0, 15, 20, 20, 15,  0,-20,
   -20,  5, 20, 35, 35, 20,  5,-20,
   -20,  5, 20, 35, 35, 20,  5,-20,
   -20,  0, 15, 20, 20, 15,  0,-20,
   -30,-10,  0,  5,  5,  0,-10,-30,
   -50,-30,-20,-20,-20,-20,-30,-50,
];

// ---------------------------------------------------------------------------
// Adjacent file masks for isolated pawn detection
// ---------------------------------------------------------------------------

const ADJACENT_FILES: [Bitboard; 8] = [
    FILE_B,                     // file A
    FILE_A | FILE_C,            // file B
    FILE_B | FILE_D,            // file C
    FILE_C | FILE_E,            // file D
    FILE_D | FILE_F,            // file E
    FILE_E | FILE_G,            // file F
    FILE_F | FILE_H,            // file G
    FILE_G,                     // file H
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Mirror a square index for black's perspective
#[inline]
fn mirror_square(sq: u8) -> u8 {
    sq ^ 56 // flip rank: rank 0 <-> rank 7
}

/// Macro to reduce repetition when evaluating PSTs for a piece type
macro_rules! eval_pst {
    ($pieces:expr, $color:expr, $mg_table:expr, $eg_table:expr, $score:expr) => {
        for sq in iter_bits($pieces) {
            let idx = if $color == Color::White { sq } else { mirror_square(sq) } as usize;
            $score += Score::new($mg_table[idx], $eg_table[idx]);
        }
    };
}

// ---------------------------------------------------------------------------
// Evaluation sub-functions (all return Score)
// ---------------------------------------------------------------------------

fn evaluate_material(board: &Board, color: Color) -> Score {
    let pieces = board.get_color_bb(color);

    let pawns = popcount(board.get_piece_bb(PieceType::Pawn) & pieces) as i32;
    let knights = popcount(board.get_piece_bb(PieceType::Knight) & pieces) as i32;
    let bishops = popcount(board.get_piece_bb(PieceType::Bishop) & pieces) as i32;
    let rooks = popcount(board.get_piece_bb(PieceType::Rook) & pieces) as i32;
    let queens = popcount(board.get_piece_bb(PieceType::Queen) & pieces) as i32;

    let mut score = Score::new(
        pawns * PAWN_SCORE.mg + knights * KNIGHT_SCORE.mg + bishops * BISHOP_SCORE.mg
            + rooks * ROOK_SCORE.mg + queens * QUEEN_SCORE.mg,
        pawns * PAWN_SCORE.eg + knights * KNIGHT_SCORE.eg + bishops * BISHOP_SCORE.eg
            + rooks * ROOK_SCORE.eg + queens * QUEEN_SCORE.eg,
    );

    // Bishop pair bonus
    if bishops >= 2 {
        score += BISHOP_PAIR;
    }

    score
}

fn evaluate_pst(board: &Board, color: Color) -> Score {
    let pieces = board.get_color_bb(color);
    let mut score = Score::ZERO;

    eval_pst!(board.get_piece_bb(PieceType::Pawn) & pieces, color, PAWN_PST_MG, PAWN_PST_EG, score);
    eval_pst!(board.get_piece_bb(PieceType::Knight) & pieces, color, KNIGHT_PST_MG, KNIGHT_PST_EG, score);
    eval_pst!(board.get_piece_bb(PieceType::Bishop) & pieces, color, BISHOP_PST_MG, BISHOP_PST_EG, score);
    eval_pst!(board.get_piece_bb(PieceType::Rook) & pieces, color, ROOK_PST_MG, ROOK_PST_EG, score);
    eval_pst!(board.get_piece_bb(PieceType::Queen) & pieces, color, QUEEN_PST_MG, QUEEN_PST_EG, score);
    eval_pst!(board.get_piece_bb(PieceType::King) & pieces, color, KING_PST_MG, KING_PST_EG, score);

    score
}

fn evaluate_pawn_structure(board: &Board, color: Color) -> Score {
    let our_pawns = board.get_piece_bb(PieceType::Pawn) & board.get_color_bb(color);
    let their_pawns = board.get_piece_bb(PieceType::Pawn) & board.get_color_bb(color.opposite());
    let mut score = Score::ZERO;

    for sq in iter_bits(our_pawns) {
        let file = get_file(sq);
        let rank = get_rank(sq);
        let file_bb = file_mask(file);

        // Doubled pawns: more than one pawn on the same file
        let pawns_on_file = popcount(our_pawns & file_bb);
        if pawns_on_file > 1 {
            score += DOUBLED_PAWN;
        }

        // Isolated pawns: no friendly pawns on adjacent files
        let adjacent = ADJACENT_FILES[file as usize];
        if (our_pawns & adjacent) == 0 {
            score += ISOLATED_PAWN;
        }

        // Passed pawns: no enemy pawns ahead on same or adjacent files
        let ahead_mask = if color == Color::White {
            !((1u64 << ((rank + 1) * 8)) - 1) | (1u64 << ((rank + 1) * 8)) - (1u64 << (rank * 8))
        } else {
            (1u64 << (rank * 8)) - 1
        };

        let blocking_files = file_bb | adjacent;
        let blocking_pawns = their_pawns & blocking_files & ahead_mask;

        if blocking_pawns == 0 {
            let bonus_rank = if color == Color::White { rank } else { 7 - rank };
            score += Score::new(
                PASSED_PAWN_MG[bonus_rank as usize],
                PASSED_PAWN_EG[bonus_rank as usize],
            );
        }
    }

    score
}

fn evaluate_mobility(board: &Board, color: Color) -> Score {
    let occupied = board.get_occupied();
    let friendly = board.get_color_bb(color);
    let mut score = Score::ZERO;

    // Knight mobility
    let knights = board.get_piece_bb(PieceType::Knight) & friendly;
    for sq in iter_bits(knights) {
        let attacks = get_knight_attacks(sq) & !friendly;
        let count = popcount(attacks) as i32;
        score += Score::new(count * KNIGHT_MOBILITY.mg, count * KNIGHT_MOBILITY.eg);
    }

    // Bishop mobility
    let bishops = board.get_piece_bb(PieceType::Bishop) & friendly;
    for sq in iter_bits(bishops) {
        let attacks = get_bishop_attacks(sq, occupied) & !friendly;
        let count = popcount(attacks) as i32;
        score += Score::new(count * BISHOP_MOBILITY.mg, count * BISHOP_MOBILITY.eg);
    }

    // Rook mobility
    let rooks = board.get_piece_bb(PieceType::Rook) & friendly;
    for sq in iter_bits(rooks) {
        let attacks = get_rook_attacks(sq, occupied) & !friendly;
        let count = popcount(attacks) as i32;
        score += Score::new(count * ROOK_MOBILITY.mg, count * ROOK_MOBILITY.eg);
    }

    // Queen mobility (lower weight to avoid overvaluing early queen development)
    let queens = board.get_piece_bb(PieceType::Queen) & friendly;
    for sq in iter_bits(queens) {
        let attacks = (get_rook_attacks(sq, occupied) | get_bishop_attacks(sq, occupied)) & !friendly;
        let count = popcount(attacks) as i32;
        score += Score::new(count * QUEEN_MOBILITY.mg, count * QUEEN_MOBILITY.eg);
    }

    score
}

// ---------------------------------------------------------------------------
// Main evaluation function
// ---------------------------------------------------------------------------

/// Main evaluation function.
/// Returns score in centipawns from the perspective of the side to move.
/// Positive = good for side to move, negative = bad.
pub fn evaluate(board: &Board) -> i32 {
    let side_to_move = board.get_side_to_move();
    let opponent = side_to_move.opposite();

    let material = evaluate_material(board, side_to_move) - evaluate_material(board, opponent);
    let pst = evaluate_pst(board, side_to_move) - evaluate_pst(board, opponent);
    let pawn_structure = evaluate_pawn_structure(board, side_to_move)
        - evaluate_pawn_structure(board, opponent);
    let mobility = evaluate_mobility(board, side_to_move) - evaluate_mobility(board, opponent);

    let total = material + pst + pawn_structure + mobility;

    // Tapered evaluation: interpolate between MG and EG based on phase
    let phase = compute_phase(board);
    (total.mg * phase + total.eg * (MAX_PHASE - phase)) / MAX_PHASE
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position() {
        let board = Board::startpos();
        let score = evaluate(&board);
        // Starting position should be roughly equal
        assert!(score.abs() < 50, "Starting position score {} too far from 0", score);
    }

    #[test]
    fn test_material_advantage() {
        // White up a queen
        let board = Board::from_fen("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let score = evaluate(&board);
        assert!(score > 800, "White up a queen should have high score: {}", score);
    }

    #[test]
    fn test_symmetry() {
        // Symmetric position should evaluate to ~0 regardless of side to move
        let white_board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
        let black_board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1").unwrap();

        let white_score = evaluate(&white_board);
        let black_score = evaluate(&black_board);

        assert!((white_score + black_score).abs() < 20,
            "Symmetric positions should have opposite scores: white={}, black={}", white_score, black_score);
    }

    #[test]
    fn test_passed_pawn() {
        // White has a passed pawn on e5
        let board = Board::from_fen("8/8/8/4P3/8/8/8/4K2k w - - 0 1").unwrap();
        let score = evaluate(&board);
        assert!(score > PAWN_VALUE, "Passed pawn should add significant value: {}", score);
    }

    #[test]
    fn test_phase_computation() {
        // Starting position: all pieces present => phase = 24
        let board = Board::startpos();
        assert_eq!(compute_phase(&board), MAX_PHASE,
            "Starting position should have max phase (24)");

        // Kings only => phase = 0
        let board = Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(compute_phase(&board), 0,
            "Kings-only position should have phase 0");
    }

    #[test]
    fn test_endgame_king_centralization() {
        // Central king should score better than corner king in a pure endgame
        // White king central vs White king in corner, both with just kings + a pawn
        let central = Board::from_fen("4k3/8/8/8/3K4/8/4P3/8 w - - 0 1").unwrap();
        let corner = Board::from_fen("4k3/8/8/8/8/8/4P3/K7 w - - 0 1").unwrap();

        let central_score = evaluate(&central);
        let corner_score = evaluate(&corner);

        assert!(central_score > corner_score,
            "Central king ({}) should score better than corner king ({}) in endgame",
            central_score, corner_score);
    }
}
