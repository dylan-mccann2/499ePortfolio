//! Search module implementing alpha-beta pruning with transposition table.
//!
//! Uses negamax with alpha-beta pruning to find the best move.

use crate::board::{Board, PieceType};
use crate::eval::{evaluate, PAWN_VALUE, KNIGHT_VALUE, BISHOP_VALUE, ROOK_VALUE, QUEEN_VALUE};
use crate::movegen::{generate_moves, is_square_attacked, Move};
use crate::tt::{TranspositionTable, ScoreType};

/// Score representing checkmate
pub const CHECKMATE_SCORE: i32 = 100_000;

/// Score representing a draw (stalemate, repetition, etc.)
pub const DRAW_SCORE: i32 = 0;

/// Maximum search depth
pub const MAX_DEPTH: u8 = 64;

/// Score threshold for mate detection
const MATE_THRESHOLD: i32 = CHECKMATE_SCORE - 1000;

/// Result of a search operation
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub nodes_searched: u64,
    pub depth: u8,
    pub tt_hits: u64,
}

/// Search state tracking
pub struct SearchState {
    pub nodes: u64,
    pub tt_hits: u64,
    pub tt: TranspositionTable,
    pub ply: u8,
}

impl SearchState {
    pub fn new() -> Self {
        SearchState {
            nodes: 0,
            tt_hits: 0,
            tt: TranspositionTable::new(64), // 64 MB default
            ply: 0,
        }
    }

    pub fn with_tt_size(size_mb: usize) -> Self {
        SearchState {
            nodes: 0,
            tt_hits: 0,
            tt: TranspositionTable::new(size_mb),
            ply: 0,
        }
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

/// Adjust mate score for storage in TT (relative to root)
#[inline]
fn score_to_tt(score: i32, ply: u8) -> i32 {
    if score > MATE_THRESHOLD {
        score + ply as i32
    } else if score < -MATE_THRESHOLD {
        score - ply as i32
    } else {
        score
    }
}

/// Adjust mate score from TT (relative to current position)
#[inline]
fn score_from_tt(score: i32, ply: u8) -> i32 {
    if score > MATE_THRESHOLD {
        score - ply as i32
    } else if score < -MATE_THRESHOLD {
        score + ply as i32
    } else {
        score
    }
}

/// Check if the current side's king is in check
pub fn is_in_check(board: &Board) -> bool {
    let us = board.get_side_to_move();
    let king_bb = board.get_piece_bb(PieceType::King) & board.get_color_bb(us);

    if king_bb == 0 {
        return false;
    }

    let king_sq = king_bb.trailing_zeros() as u8;
    is_square_attacked(board, king_sq, us.opposite())
}

/// Generate only legal moves (filter out moves that leave king in check)
pub fn generate_legal_moves(board: &mut Board) -> Vec<Move> {
    let pseudo_moves = generate_moves(board);
    let mut legal_moves = Vec::with_capacity(pseudo_moves.len());

    for mv in pseudo_moves {
        let unmove = board.make_move(&mv);

        // After making the move, check if our king is attacked
        // Note: side_to_move has switched, so we check if opponent can attack our king
        let us = board.get_side_to_move().opposite(); // The side that just moved
        let king_bb = board.get_piece_bb(PieceType::King) & board.get_color_bb(us);

        if king_bb != 0 {
            let king_sq = king_bb.trailing_zeros() as u8;
            if !is_square_attacked(board, king_sq, board.get_side_to_move()) {
                legal_moves.push(mv);
            }
        }

        board.unmake_move(&unmove);
    }

    legal_moves
}

/// MVV-LVA (Most Valuable Victim - Least Valuable Attacker) scoring for move ordering
fn mvv_lva_score(board: &Board, mv: &Move) -> i32 {
    let victim = board.piece_at(mv.to);
    let attacker = board.piece_at(mv.from);

    let victim_value = match victim {
        PieceType::Pawn => PAWN_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::Queen => QUEEN_VALUE,
        PieceType::King => CHECKMATE_SCORE,
        PieceType::None => 0,
    };

    let attacker_value = match attacker {
        PieceType::Pawn => PAWN_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::Queen => QUEEN_VALUE,
        PieceType::King => 0, // King captures are fine
        PieceType::None => 0,
    };

    // Higher score = better capture (high value victim, low value attacker)
    if victim_value > 0 {
        victim_value * 10 - attacker_value
    } else if mv.promotion.is_some() {
        // Promotions are good
        QUEEN_VALUE
    } else {
        0
    }
}

/// Order moves for better alpha-beta pruning efficiency
/// TT move is tried first if available
fn order_moves(board: &Board, moves: &mut Vec<Move>, tt_move: Option<Move>) {
    // Put TT move first if it exists
    if let Some(tt_mv) = tt_move {
        if let Some(pos) = moves.iter().position(|m| m.from == tt_mv.from && m.to == tt_mv.to && m.promotion == tt_mv.promotion) {
            moves.swap(0, pos);
            // Sort rest by MVV-LVA
            moves[1..].sort_by(|a, b| {
                let score_a = mvv_lva_score(board, a);
                let score_b = mvv_lva_score(board, b);
                score_b.cmp(&score_a)
            });
            return;
        }
    }

    // No TT move, sort all by MVV-LVA
    moves.sort_by(|a, b| {
        let score_a = mvv_lva_score(board, a);
        let score_b = mvv_lva_score(board, b);
        score_b.cmp(&score_a)
    });
}

/// Quiescence search to avoid horizon effect
/// Only searches captures to reach a "quiet" position
fn quiescence(board: &mut Board, mut alpha: i32, beta: i32, state: &mut SearchState) -> i32 {
    state.nodes += 1;

    // Stand pat score
    let stand_pat = evaluate(board);

    if stand_pat >= beta {
        return beta;
    }

    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Generate and filter to only captures
    let mut moves = generate_legal_moves(board);
    moves.retain(|mv| board.piece_at(mv.to) != PieceType::None || mv.promotion.is_some());
    order_moves(board, &mut moves, None);

    for mv in moves {
        let unmove = board.make_move(&mv);
        state.ply += 1;
        let score = -quiescence(board, -beta, -alpha, state);
        state.ply -= 1;
        board.unmake_move(&unmove);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// Negamax search with alpha-beta pruning and transposition table
fn negamax(
    board: &mut Board,
    depth: u8,
    mut alpha: i32,
    beta: i32,
    state: &mut SearchState,
) -> i32 {
    state.nodes += 1;

    let alpha_orig = alpha;
    let hash = board.get_hash();

    // Probe transposition table
    let tt_move = if let Some(entry) = state.tt.probe(hash) {
        state.tt_hits += 1;

        if entry.depth >= depth {
            let score = score_from_tt(entry.score, state.ply);

            match entry.score_type {
                ScoreType::Exact => return score,
                ScoreType::LowerBound => {
                    if score > alpha {
                        alpha = score;
                    }
                }
                ScoreType::UpperBound => {
                    if score < beta {
                        return score.min(beta);
                    }
                }
            }

            if alpha >= beta {
                return score;
            }
        }

        entry.best_move
    } else {
        None
    };

    // Base case: leaf node
    if depth == 0 {
        return quiescence(board, alpha, beta, state);
    }

    let mut legal_moves = generate_legal_moves(board);

    // Check for checkmate or stalemate
    if legal_moves.is_empty() {
        if is_in_check(board) {
            // Checkmate - return negative score (bad for current player)
            return -CHECKMATE_SCORE + state.ply as i32;
        } else {
            // Stalemate
            return DRAW_SCORE;
        }
    }

    // Order moves for better pruning (TT move first)
    order_moves(board, &mut legal_moves, tt_move);

    let mut best_score = i32::MIN + 1;
    let mut best_move = None;

    for mv in legal_moves {
        let unmove = board.make_move(&mv);
        state.ply += 1;
        let score = -negamax(board, depth - 1, -beta, -alpha, state);
        state.ply -= 1;
        board.unmake_move(&unmove);

        if score > best_score {
            best_score = score;
            best_move = Some(mv);
        }

        if score > alpha {
            alpha = score;
        }

        // Beta cutoff
        if alpha >= beta {
            break;
        }
    }

    // Store in transposition table
    let score_type = if best_score <= alpha_orig {
        ScoreType::UpperBound
    } else if best_score >= beta {
        ScoreType::LowerBound
    } else {
        ScoreType::Exact
    };

    state.tt.store(
        hash,
        depth,
        score_to_tt(best_score, state.ply),
        score_type,
        best_move,
    );

    best_score
}

/// Search for the best move at a given depth
pub fn search(board: &mut Board, depth: u8) -> SearchResult {
    let mut state = SearchState::new();
    search_with_state(board, depth, &mut state)
}

/// Search with an existing state (preserves TT between searches)
pub fn search_with_state(board: &mut Board, depth: u8, state: &mut SearchState) -> SearchResult {
    state.nodes = 0;
    state.tt_hits = 0;
    state.ply = 0;
    state.tt.new_search();

    let mut best_move = None;
    let mut best_score = i32::MIN + 1;
    let mut alpha = i32::MIN + 1;
    let beta = i32::MAX;

    let hash = board.get_hash();
    let tt_move = state.tt.get_best_move(hash);

    let mut legal_moves = generate_legal_moves(board);

    if legal_moves.is_empty() {
        // No legal moves - checkmate or stalemate
        let score = if is_in_check(board) {
            -CHECKMATE_SCORE
        } else {
            DRAW_SCORE
        };
        return SearchResult {
            best_move: None,
            score,
            nodes_searched: 1,
            depth,
            tt_hits: 0,
        };
    }

    // Order moves (TT move first)
    order_moves(board, &mut legal_moves, tt_move);

    for mv in &legal_moves {
        let unmove = board.make_move(mv);
        state.ply = 1;
        let score = -negamax(board, depth - 1, -beta, -alpha, state);
        state.ply = 0;
        board.unmake_move(&unmove);

        if score > best_score {
            best_score = score;
            best_move = Some(*mv);
        }

        if score > alpha {
            alpha = score;
        }
    }

    // Store root position in TT
    state.tt.store(hash, depth, best_score, ScoreType::Exact, best_move);

    SearchResult {
        best_move,
        score: best_score,
        nodes_searched: state.nodes,
        depth,
        tt_hits: state.tt_hits,
    }
}

/// Iterative deepening search up to a maximum depth
pub fn search_iterative(board: &mut Board, max_depth: u8) -> SearchResult {
    let mut state = SearchState::new();
    search_iterative_with_state(board, max_depth, &mut state)
}

/// Iterative deepening with existing state (preserves TT)
pub fn search_iterative_with_state(board: &mut Board, max_depth: u8, state: &mut SearchState) -> SearchResult {
    let mut best_result = SearchResult {
        best_move: None,
        score: 0,
        nodes_searched: 0,
        depth: 0,
        tt_hits: 0,
    };

    for depth in 1..=max_depth {
        let result = search_with_state(board, depth, state);
        best_result = SearchResult {
            best_move: result.best_move.or(best_result.best_move),
            score: result.score,
            nodes_searched: best_result.nodes_searched + result.nodes_searched,
            depth,
            tt_hits: best_result.tt_hits + result.tt_hits,
        };

        // Early exit if we found a checkmate
        if result.score.abs() > CHECKMATE_SCORE - 100 {
            break;
        }
    }

    best_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_position() {
        let mut board = Board::startpos();
        let result = search(&mut board, 3);

        assert!(result.best_move.is_some());
        assert!(result.nodes_searched > 0);
    }

    #[test]
    fn test_checkmate_in_one() {
        // Back rank mate position: White to move, Qh7#
        let mut board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R3K3 w Q - 0 1").unwrap();
        let result = search(&mut board, 2);

        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        // Should find Ra8#
        assert_eq!(mv.to, 56); // a8
    }

    #[test]
    fn test_avoid_checkmate() {
        // Black king in danger, needs to escape
        let mut board = Board::from_fen("k7/8/1K6/8/8/8/8/R7 b - - 0 1").unwrap();
        let result = search(&mut board, 3);

        // Black should have a move to escape
        assert!(result.best_move.is_some());
    }

    #[test]
    fn test_capture_free_piece() {
        // White can capture undefended black queen on c5 with knight on e4
        let mut board = Board::from_fen("k7/8/8/2q5/4N3/8/8/K7 w - - 0 1").unwrap();
        let result = search(&mut board, 3);

        assert!(result.best_move.is_some());
        let mv = result.best_move.unwrap();
        // Knight should capture queen: e4 (28) to c5 (34)
        assert_eq!(mv.from, 28, "Expected knight from e4");
        assert_eq!(mv.to, 34, "Expected capture on c5");
        // After capturing, white has knight vs nothing (~320+ centipawns advantage)
        assert!(result.score > 300, "Expected winning score, got {}", result.score);
    }

    #[test]
    fn test_stalemate_detection() {
        // Stalemate position - Black to move but no legal moves
        let mut board = Board::from_fen("k7/8/1K6/8/8/8/8/8 b - - 0 1").unwrap();
        let moves = generate_legal_moves(&mut board);

        if moves.is_empty() && !is_in_check(&board) {
            // This is stalemate
            let result = search(&mut board, 1);
            assert_eq!(result.score, DRAW_SCORE);
        }
    }

    #[test]
    fn test_legal_move_generation() {
        let mut board = Board::startpos();
        let moves = generate_legal_moves(&mut board);

        // Starting position has 20 legal moves
        assert_eq!(moves.len(), 20);
    }

    #[test]
    fn test_iterative_deepening() {
        let mut board = Board::startpos();
        let result = search_iterative(&mut board, 4);

        assert!(result.best_move.is_some());
        assert_eq!(result.depth, 4);
        assert!(result.nodes_searched > 0);
    }

    #[test]
    fn test_tt_improves_search() {
        let mut board = Board::startpos();

        // Search without TT reuse
        let result1 = search(&mut board, 4);

        // Search with same state (TT preserved)
        let mut state = SearchState::new();
        let _ = search_with_state(&mut board, 4, &mut state);
        let result2 = search_with_state(&mut board, 4, &mut state);

        // Second search should have TT hits
        assert!(result2.tt_hits > 0, "Expected TT hits in second search");

        // Both should find the same best move
        assert_eq!(result1.best_move, result2.best_move);
    }
}
