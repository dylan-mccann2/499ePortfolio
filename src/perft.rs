use crate::board::{Board, PieceType};
use crate::movegen::{generate_moves, is_square_attacked, Move};

pub fn perft(board: &mut Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    let moves = generate_moves(board);
    let mut nodes = 0;

    for mv in moves {
        let unmove = board.make_move(&mv);

        // Check if the move leaves our king in check (illegal move)
        let us = board.get_side_to_move().opposite(); // We just switched sides
        let king_bb = board.get_piece_bb(PieceType::King) & board.get_color_bb(us);
        if king_bb != 0 {
            let king_sq = king_bb.trailing_zeros() as u8;
            if !is_square_attacked(board, king_sq, board.get_side_to_move()) {
                nodes += perft(board, depth - 1);
            }
        }

        board.unmake_move(&unmove);
    }

    nodes
}

pub fn perft_divide(board: &mut Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    let moves = generate_moves(board);
    let mut total = 0;

    for mv in moves {
        let unmove = board.make_move(&mv);

        // Check if the move leaves our king in check (illegal move)
        let us = board.get_side_to_move().opposite();
        let king_bb = board.get_piece_bb(PieceType::King) & board.get_color_bb(us);
        if king_bb != 0 {
            let king_sq = king_bb.trailing_zeros() as u8;
            if !is_square_attacked(board, king_sq, board.get_side_to_move()) {
                let nodes = perft(board, depth - 1);
                println!("{}: {}", move_to_uci(&mv), nodes);
                total += nodes;
            }
        }

        board.unmake_move(&unmove);
    }

    println!("\nTotal: {}", total);
    total
}

fn move_to_uci(mv: &Move) -> String {
    use crate::board::bitboard::{get_file, get_rank};

    let from_file = (b'a' + get_file(mv.from)) as char;
    let from_rank = (b'1' + get_rank(mv.from)) as char;
    let to_file = (b'a' + get_file(mv.to)) as char;
    let to_rank = (b'1' + get_rank(mv.to)) as char;

    let mut uci = format!("{}{}{}{}", from_file, from_rank, to_file, to_rank);

    if let Some(promo) = mv.promotion {
        let promo_char = match promo {
            PieceType::Queen => 'q',
            PieceType::Rook => 'r',
            PieceType::Bishop => 'b',
            PieceType::Knight => 'n',
            _ => '?',
        };
        uci.push(promo_char);
    }

    uci
}

#[cfg(test)]
mod tests {

    use super::*;

    // Standard perft test positions
    // https://www.chessprogramming.org/Perft_Results

    #[test]
    fn perft_startpos_depth_1() {
        let mut board = Board::startpos();
        assert_eq!(perft(&mut board, 1), 20);
    }

    #[test]
    fn perft_startpos_depth_2() {
        let mut board = Board::startpos();
        assert_eq!(perft(&mut board, 2), 400);
    }

    #[test]
    fn perft_startpos_depth_3() {
        let mut board = Board::startpos();
        assert_eq!(perft(&mut board, 3), 8902);
    }

    #[test]
    fn perft_startpos_depth_4() {
        let mut board = Board::startpos();
        assert_eq!(perft(&mut board, 4), 197281);
    }

    #[test]
    fn perft_startpos_depth_5() {
        let mut board = Board::startpos();
        assert_eq!(perft(&mut board, 5), 4865609);
    }

    // Position 2: Kiwipete
    // r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -
    #[test]
    fn perft_kiwipete_depth_1() {
        let mut board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 1), 48);
    }

    #[test]
    fn perft_kiwipete_depth_2() {
        let mut board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 2), 2039);
    }

    #[test]
    fn perft_kiwipete_depth_3() {
        let mut board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 3), 97862);
    }

    #[test]
    fn perft_kiwipete_depth_4() {
        let mut board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 4), 4085603);
    }

    // Position 3: Endgame
    // 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -
    #[test]
    fn perft_endgame_depth_1() {
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft(&mut board, 1), 14);
    }

    #[test]
    fn perft_endgame_depth_2() {
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft(&mut board, 2), 191);
    }

    #[test]
    fn perft_endgame_depth_3() {
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft(&mut board, 3), 2812);
    }

    #[test]
    fn perft_endgame_depth_4() {
        let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").unwrap();
        assert_eq!(perft(&mut board, 4), 43238);
    }

    // Position 4: Mirror positions
    // r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1
    #[test]
    fn perft_position4_depth_1() {
        let mut board = Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 1), 6);
    }

    #[test]
    fn perft_position4_depth_2() {
        let mut board = Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 2), 264);
    }

    #[test]
    fn perft_position4_depth_3() {
        let mut board = Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1").unwrap();
        assert_eq!(perft(&mut board, 3), 9467);
    }

    // Position 5
    // rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8
    #[test]
    fn perft_position5_depth_1() {
        let mut board = Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").unwrap();
        assert_eq!(perft(&mut board, 1), 44);
    }

    #[test]
    fn perft_position5_depth_2() {
        let mut board = Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").unwrap();
        assert_eq!(perft(&mut board, 2), 1486);
    }

    #[test]
    fn perft_position5_depth_3() {
        let mut board = Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").unwrap();
        assert_eq!(perft(&mut board, 3), 62379);
    }

    // Position 6
    // r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10
    #[test]
    fn perft_position6_depth_1() {
        let mut board = Board::from_fen("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10").unwrap();
        assert_eq!(perft(&mut board, 1), 46);
    }

    #[test]
    fn perft_position6_depth_2() {
        let mut board = Board::from_fen("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10").unwrap();
        assert_eq!(perft(&mut board, 2), 2079);
    }

    #[test]
    fn perft_position6_depth_3() {
        let mut board = Board::from_fen("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10").unwrap();
        assert_eq!(perft(&mut board, 3), 89890);
    }
}
