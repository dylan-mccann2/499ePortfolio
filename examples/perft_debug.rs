use chess::board::Board;
use chess::perft::{perft, perft_divide};
use chess::movegen::{generate_moves, is_square_attacked};
use chess::board::{Color, PieceType};

fn move_to_str(mv: &chess::movegen::Move) -> String {
    let from_file = (b'a' + (mv.from % 8)) as char;
    let from_rank = (b'1' + (mv.from / 8)) as char;
    let to_file = (b'a' + (mv.to % 8)) as char;
    let to_rank = (b'1' + (mv.to / 8)) as char;
    let promo = match mv.promotion {
        Some(PieceType::Queen) => "q",
        Some(PieceType::Rook) => "r",
        Some(PieceType::Bishop) => "b",
        Some(PieceType::Knight) => "n",
        _ => ""
    };
    format!("{}{}{}{}{}", from_file, from_rank, to_file, to_rank, promo)
}

fn get_legal_moves(board: &Board) -> Vec<chess::movegen::Move> {
    let moves = generate_moves(board);
    let us = board.get_side_to_move();
    let them = us.opposite();

    moves.into_iter().filter(|mv| {
        let mut board_copy = board.clone();
        board_copy.make_move(mv);
        let king_bb = board_copy.get_piece_bb(PieceType::King) & board_copy.get_color_bb(us);
        if king_bb != 0 {
            let king_sq = king_bb.trailing_zeros() as u8;
            !is_square_attacked(&board_copy, king_sq, them)
        } else {
            false
        }
    }).collect()
}

fn main() {
    // Test for pin-related issues by creating positions with pins
    println!("=== Testing pin scenarios ===\n");

    // Position with a pinned piece
    // White rook pins black bishop to black king
    let pinned = Board::from_fen("4k3/8/4b3/8/8/8/8/4R2K b - - 0 1").unwrap();
    println!("Pinned bishop test (rook pins bishop to king along e-file):");
    println!("  Black has {} legal moves", get_legal_moves(&pinned).len());
    println!("  Legal moves:");
    for mv in get_legal_moves(&pinned) {
        println!("    {}", move_to_str(&mv));
    }
    // Expected: only king moves (Kd8, Kf8, Kd7, Kf7) = 4 moves
    // Bishop cannot move because any diagonal move leaves the e-file, exposing king to rook

    // Test en passant with pin (famous bug case)
    // In this position, e5xd6 is LEGAL because it only exposes the BLACK king (discovered check for white!)
    println!("\n=== Testing en passant discovered check (legal) ===");
    let ep_check = Board::from_fen("8/8/8/1k1pP2R/8/8/8/4K3 w - d6 0 1").unwrap();
    println!("Position: 8/8/8/1k1pP2R/8/8/8/4K3 w - d6 0 1");
    println!("EP square: {:?}", ep_check.get_en_passant_square());
    let legal = get_legal_moves(&ep_check);
    println!("White legal moves: {}", legal.len());
    for mv in &legal {
        println!("  {}", move_to_str(mv));
    }
    // e5xd6 IS legal - it's a discovered check on the BLACK king (good for white!)
    let has_ep = legal.iter().any(|m| m.from == 36 && m.to == 43); // e5 to d6
    println!("Has en passant move (e5xd6)? {} (should be TRUE - discovered check!)", has_ep);

    // Another EP pin test
    println!("\n=== Another en passant pin test ===");
    let ep_pin2 = Board::from_fen("8/8/8/KPp4r/8/8/8/4k3 w - c6 0 1").unwrap();
    println!("Position: 8/8/8/KPp4r/8/8/8/4k3 w - c6 0 1");
    println!("EP square: {:?}", ep_pin2.get_en_passant_square());
    let legal2 = get_legal_moves(&ep_pin2);
    println!("White legal moves: {}", legal2.len());
    for mv in &legal2 {
        println!("  {}", move_to_str(mv));
    }
    let has_ep2 = legal2.iter().any(|m| m.from == 33 && m.to == 42); // b5 to c6
    println!("Has en passant move (b5xc6)? {} (should be false!)", has_ep2);

    // Run the failing positions one more time with summary
    println!("\n=== Summary ===");
    let mut p4 = Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1").unwrap();
    let mut p5 = Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").unwrap();

    println!("Position 4: perft(3) = {} (expected 9467)", perft(&mut p4, 3));
    println!("Position 5: perft(3) = {} (expected 62379)", perft(&mut p5, 3));
}
