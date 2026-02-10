//! UCI (Universal Chess Interface) protocol implementation.
//!
//! Provides a standard interface for chess GUIs to communicate with the engine.

use std::io::{self, BufRead, Write};

use crate::board::{Board, Color, PieceType, get_file, get_rank};
use crate::movegen::Move;
use crate::openings::{Opening, import_openings, lookup_opening};
use crate::search::{search_with_state, is_in_check, generate_legal_moves, SearchState, CHECKMATE_SCORE};

use mongodb::{Client, Collection};
use tokio::runtime::Runtime;

const ENGINE_NAME: &str = "ChessEngine";
const ENGINE_AUTHOR: &str = "Dylan";
const DEFAULT_HASH_SIZE: usize = 16; // 16 MB default

/// UCI interface state
pub struct Uci {
    board: Board,
    debug: bool,
    search_state: SearchState,
    runtime: Runtime,
    db_client: Option<Client>,
    openings: Option<Collection<Opening>>,
}

impl Uci {
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("failed to create tokio runtime");

        // Try to connect to MongoDB for opening lookups
        let db_client = runtime.block_on(async {
            Client::with_uri_str("mongodb://localhost:27017").await.ok()
        });

        let openings = db_client.as_ref().map(|c| {
            c.database("chess").collection::<Opening>("openings")
        });

        Uci {
            board: Board::startpos(),
            debug: false,
            search_state: SearchState::with_tt_size(DEFAULT_HASH_SIZE),
            runtime,
            db_client,
            openings,
        }
    }

    /// Run the UCI main loop
    pub fn run(&mut self) {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if self.debug {
                eprintln!("info string received: {}", line);
            }

            let response = self.handle_command(line);

            if let Some(quit) = response {
                if quit {
                    break;
                }
            }

            stdout.flush().unwrap();
        }
    }

    /// Handle a single UCI command, returns Some(true) if should quit
    fn handle_command(&mut self, line: &str) -> Option<bool> {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            return None;
        }

        match tokens[0] {
            "uci" => {
                self.cmd_uci();
                None
            }
            "debug" => {
                self.cmd_debug(&tokens);
                None
            }
            "isready" => {
                self.cmd_isready();
                None
            }
            "setoption" => {
                self.cmd_setoption(&tokens);
                None
            }
            "ucinewgame" => {
                self.cmd_ucinewgame();
                None
            }
            "position" => {
                self.cmd_position(&tokens);
                None
            }
            "go" => {
                self.cmd_go(&tokens);
                None
            }
"quit" => Some(true),
            // Non-standard but useful commands
            "d" | "display" => {
                self.cmd_display();
                None
            }
            "perft" => {
                self.cmd_perft(&tokens);
                None
            }
            "importopenings" => {
                self.cmd_import_openings(&tokens);
                None
            }
            _ => {
                if self.debug {
                    eprintln!("info string unknown command: {}", tokens[0]);
                }
                None
            }
        }
    }

    /// Handle 'uci' command - identify the engine
    fn cmd_uci(&self) {
        println!("id name {}", ENGINE_NAME);
        println!("id author {}", ENGINE_AUTHOR);
        println!("option name Hash type spin default {} min 1 max 1024", DEFAULT_HASH_SIZE);
        println!("uciok");
    }

    /// Handle 'debug' command
    fn cmd_debug(&mut self, tokens: &[&str]) {
        if tokens.len() > 1 {
            match tokens[1] {
                "on" => self.debug = true,
                "off" => self.debug = false,
                _ => {}
            }
        }
    }

    /// Handle 'isready' command
    fn cmd_isready(&self) {
        println!("readyok");
    }

    /// Handle 'setoption' command
    fn cmd_setoption(&mut self, tokens: &[&str]) {
        // Format: setoption name <name> [value <value>]
        let mut name_idx = None;
        let mut value_idx = None;

        for (i, &token) in tokens.iter().enumerate() {
            if token == "name" && name_idx.is_none() {
                name_idx = Some(i + 1);
            } else if token == "value" {
                value_idx = Some(i + 1);
            }
        }

        if let Some(ni) = name_idx {
            let name_end = value_idx.map(|v| v - 1).unwrap_or(tokens.len());
            let name = tokens[ni..name_end].join(" ").to_lowercase();

            match name.as_str() {
                "hash" => {
                    if let Some(vi) = value_idx {
                        if let Ok(size) = tokens[vi].parse::<usize>() {
                            let size = size.clamp(1, 1024);
                            self.search_state = SearchState::with_tt_size(size);
                            if self.debug {
                                eprintln!("info string Hash set to {} MB", size);
                            }
                        }
                    }
                }
                _ => {
                    if self.debug {
                        eprintln!("info string unknown option: {}", name);
                    }
                }
            }
        }
    }

    /// Handle 'ucinewgame' command
    fn cmd_ucinewgame(&mut self) {
        self.board = Board::startpos();
        self.search_state.tt.clear();
    }

    /// Handle 'position' command
    fn cmd_position(&mut self, tokens: &[&str]) {
        if tokens.len() < 2 {
            return;
        }

        let mut idx = 1;

        // Parse position
        if tokens[idx] == "startpos" {
            self.board = Board::startpos();
            idx += 1;
        } else if tokens[idx] == "fen" {
            idx += 1;
            // Collect FEN string (6 parts)
            let mut fen_parts = Vec::new();
            while idx < tokens.len() && tokens[idx] != "moves" {
                fen_parts.push(tokens[idx]);
                idx += 1;
            }
            let fen = fen_parts.join(" ");
            match Board::from_fen(&fen) {
                Ok(board) => self.board = board,
                Err(e) => {
                    if self.debug {
                        eprintln!("info string invalid fen: {:?}", e);
                    }
                    return;
                }
            }
        }

        // Parse moves
        if idx < tokens.len() && tokens[idx] == "moves" {
            idx += 1;
            while idx < tokens.len() {
                if let Some(mv) = self.parse_move(tokens[idx]) {
                    self.board.make_move(&mv);
                } else if self.debug {
                    eprintln!("info string invalid move: {}", tokens[idx]);
                }
                idx += 1;
            }
        }
    }

    /// Parse a move in UCI notation (e.g., "e2e4", "e7e8q")
    fn parse_move(&self, move_str: &str) -> Option<Move> {
        if move_str.len() < 4 {
            return None;
        }

        let from = algebraic_to_square(&move_str[0..2])?;
        let to = algebraic_to_square(&move_str[2..4])?;

        let promotion = if move_str.len() > 4 {
            match move_str.chars().nth(4)? {
                'q' => Some(crate::board::PieceType::Queen),
                'r' => Some(crate::board::PieceType::Rook),
                'b' => Some(crate::board::PieceType::Bishop),
                'n' => Some(crate::board::PieceType::Knight),
                _ => None,
            }
        } else {
            None
        };

        Some(Move { from, to, promotion })
    }

    /// Handle 'go' command
    fn cmd_go(&mut self, tokens: &[&str]) {
        let mut depth: Option<u8> = None;
        let mut infinite = false;

        let mut i = 1;
        while i < tokens.len() {
            match tokens[i] {
                "depth" => {
                    if i + 1 < tokens.len() {
                        depth = tokens[i + 1].parse().ok();
                        i += 1;
                    }
                }
                "infinite" => {
                    infinite = true;
                }
                _ => {}
            }
            i += 1;
        }

        // Determine search depth
        let search_depth = if let Some(d) = depth {
            d
        } else if infinite {
            6
        } else {
            5
        };

        // Perform search
        self.do_search(search_depth);
    }

    /// Perform the actual search and output results
    fn do_search(&mut self, depth: u8) {
        let mut total_nodes = 0u64;
        let mut last_best_move = None;

        // Iterative deepening with info output
        for d in 1..=depth {
            let result = search_with_state(&mut self.board, d, &mut self.search_state);
            total_nodes += result.nodes_searched;

            // Output info
            let score_str = if result.score.abs() > CHECKMATE_SCORE - 100 {
                let mate_in = (CHECKMATE_SCORE - result.score.abs() + 1) / 2;
                if result.score > 0 {
                    format!("score mate {}", mate_in)
                } else {
                    format!("score mate -{}", mate_in)
                }
            } else {
                format!("score cp {}", result.score)
            };

            let hashfull = self.search_state.tt.hashfull();

            print!("info depth {} {} nodes {} hashfull {}", d, score_str, total_nodes, hashfull);

            // Print principal variation (just the best move for now)
            if let Some(ref mv) = result.best_move {
                print!(" pv {}", self.format_move(mv));
                last_best_move = Some(*mv);
            }
            println!();

            // If we found a checkmate, no need to search deeper
            if result.score.abs() > CHECKMATE_SCORE - 100 {
                break;
            }
        }

        // Output best move
        if let Some(mv) = last_best_move {
            println!("bestmove {}", self.format_move(&mv));
        } else {
            // No legal moves - output null move or first legal move
            let legal_moves = generate_legal_moves(&mut self.board);
            if let Some(mv) = legal_moves.first() {
                println!("bestmove {}", self.format_move(mv));
            } else {
                // Checkmate or stalemate
                println!("bestmove 0000");
            }
        }
    }

    /// Format a move in UCI notation
    fn format_move(&self, mv: &Move) -> String {
        let mut s = String::with_capacity(5);
        s.push_str(&square_to_algebraic(mv.from));
        s.push_str(&square_to_algebraic(mv.to));
        if let Some(promo) = mv.promotion {
            s.push(match promo {
                crate::board::PieceType::Queen => 'q',
                crate::board::PieceType::Rook => 'r',
                crate::board::PieceType::Bishop => 'b',
                crate::board::PieceType::Knight => 'n',
                _ => 'q',
            });
        }
        s
    }

    /// Handle 'd' command - display the board (non-standard, for debugging)
    fn cmd_display(&self) {
        println!();
        println!("  +---+---+---+---+---+---+---+---+");
        for rank in (0..8).rev() {
            print!("{} |", rank + 1);
            for file in 0..8 {
                let sq = rank * 8 + file;
                let piece = self.board.piece_at(sq as u8);
                let is_white = (self.board.get_color_bb(crate::board::Color::White)
                    & (1u64 << sq))
                    != 0;

                let c = match piece {
                    crate::board::PieceType::Pawn => {
                        if is_white {
                            'P'
                        } else {
                            'p'
                        }
                    }
                    crate::board::PieceType::Knight => {
                        if is_white {
                            'N'
                        } else {
                            'n'
                        }
                    }
                    crate::board::PieceType::Bishop => {
                        if is_white {
                            'B'
                        } else {
                            'b'
                        }
                    }
                    crate::board::PieceType::Rook => {
                        if is_white {
                            'R'
                        } else {
                            'r'
                        }
                    }
                    crate::board::PieceType::Queen => {
                        if is_white {
                            'Q'
                        } else {
                            'q'
                        }
                    }
                    crate::board::PieceType::King => {
                        if is_white {
                            'K'
                        } else {
                            'k'
                        }
                    }
                    crate::board::PieceType::None => '.',
                };
                print!(" {} |", c);
            }
            println!();
            println!("  +---+---+---+---+---+---+---+---+");
        }
        println!("    a   b   c   d   e   f   g   h");
        println!();

        let side = if self.board.get_side_to_move() == crate::board::Color::White {
            "White"
        } else {
            "Black"
        };
        println!("Side to move: {}", side);

        let in_check = is_in_check(&self.board);
        if in_check {
            println!("In check!");
        }

        let legal_moves = generate_legal_moves(&mut self.board.clone());
        println!("Legal moves: {}", legal_moves.len());

        // Look up opening name from database
        if let Some(ref collection) = self.openings {
            let fen = self.board.to_fen();
            if let Some(name) = self.runtime.block_on(lookup_opening(collection, &fen)) {
                println!("Opening: {}", name);
            }
        }
    }

    /// Ensure the MongoDB client and openings collection are initialized.
    fn ensure_db(&mut self) -> bool {
        if self.db_client.is_none() {
            self.db_client = self.runtime.block_on(async {
                Client::with_uri_str("mongodb://localhost:27017").await.ok()
            });
            self.openings = self.db_client.as_ref().map(|c| {
                c.database("chess").collection::<Opening>("openings")
            });
        }
        self.openings.is_some()
    }

    /// Handle 'importopenings <dir>' command (non-standard)
    fn cmd_import_openings(&mut self, tokens: &[&str]) {
        if tokens.len() < 2 {
            eprintln!("usage: importopenings <directory>");
            return;
        }

        if !self.ensure_db() {
            eprintln!("error: failed to connect to MongoDB at mongodb://localhost:27017");
            return;
        }

        let dir = std::path::Path::new(tokens[1]);
        let collection = self.openings.clone().unwrap();
        match self.runtime.block_on(import_openings(dir, &collection)) {
            Ok(()) => {}
            Err(e) => eprintln!("import failed: {}", e),
        }
    }

    /// Handle 'perft' command (non-standard)
    fn cmd_perft(&mut self, tokens: &[&str]) {
        let depth = if tokens.len() > 1 {
            tokens[1].parse().unwrap_or(1)
        } else {
            1
        };

        let start = std::time::Instant::now();
        let nodes = crate::perft::perft(&mut self.board, depth);
        let elapsed = start.elapsed();

        println!("Nodes: {}", nodes);
        println!(
            "Time: {:.3}s ({:.0} nps)",
            elapsed.as_secs_f64(),
            nodes as f64 / elapsed.as_secs_f64()
        );
    }
}

impl Default for Uci {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert algebraic notation to square index
fn algebraic_to_square(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }

    let file = bytes[0].wrapping_sub(b'a');
    let rank = bytes[1].wrapping_sub(b'1');

    if file < 8 && rank < 8 {
        Some(rank * 8 + file)
    } else {
        None
    }
}

/// Convert square index to algebraic notation
fn square_to_algebraic(sq: u8) -> String {
    let file = (sq % 8) as u8;
    let rank = (sq / 8) as u8;
    let mut s = String::with_capacity(2);
    s.push((b'a' + file) as char);
    s.push((b'1' + rank) as char);
    s
}

/// Convert a move in Standard Algebraic Notation (SAN) to UCI notation.
///
/// Requires the current board position to resolve ambiguity, since SAN
/// encodes only the destination and piece type (e.g. "Nf3") while UCI
/// needs both source and destination squares (e.g. "g1f3").
///
/// Supports: piece moves (`Nf3`), pawn moves (`e4`), captures (`exd5`, `Nxf3`),
/// disambiguation (`Nbd2`, `R1d1`), promotion (`e8=Q`), castling (`O-O`, `O-O-O`),
/// and check/checkmate suffixes (`+`, `#`).
pub fn algebraic_to_uci(board: &mut Board, san: &str) -> Option<String> {
    let mv = parse_san(board, san)?;
    let mut s = String::with_capacity(5);
    s.push_str(&square_to_algebraic(mv.from));
    s.push_str(&square_to_algebraic(mv.to));
    if let Some(promo) = mv.promotion {
        s.push(match promo {
            PieceType::Queen => 'q',
            PieceType::Rook => 'r',
            PieceType::Bishop => 'b',
            PieceType::Knight => 'n',
            _ => return None,
        });
    }
    Some(s)
}

/// Parse a move in Standard Algebraic Notation into a Move struct.
pub fn parse_san(board: &mut Board, san: &str) -> Option<Move> {
    let legal_moves = generate_legal_moves(board);

    // Strip check/checkmate indicators
    let san = san.trim_end_matches(['+', '#']);

    // Handle castling
    if san == "O-O" || san == "0-0" {
        let king_sq = if board.get_side_to_move() == Color::White { 4u8 } else { 60u8 };
        let to_sq = if board.get_side_to_move() == Color::White { 6u8 } else { 62u8 };
        return legal_moves.into_iter().find(|m| m.from == king_sq && m.to == to_sq);
    }
    if san == "O-O-O" || san == "0-0-0" {
        let king_sq = if board.get_side_to_move() == Color::White { 4u8 } else { 60u8 };
        let to_sq = if board.get_side_to_move() == Color::White { 2u8 } else { 58u8 };
        return legal_moves.into_iter().find(|m| m.from == king_sq && m.to == to_sq);
    }

    let chars: Vec<char> = san.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Determine piece type from leading uppercase letter
    let (piece, start) = if chars[0].is_ascii_uppercase() {
        let p = match chars[0] {
            'K' => PieceType::King,
            'Q' => PieceType::Queen,
            'R' => PieceType::Rook,
            'B' => PieceType::Bishop,
            'N' => PieceType::Knight,
            _ => return None,
        };
        (p, 1)
    } else {
        (PieceType::Pawn, 0)
    };

    // Strip capture indicator
    let rest: Vec<char> = chars[start..].iter().copied().filter(|&c| c != 'x').collect();

    // Split off promotion suffix (e.g. "=Q")
    let (rest, promotion) = if let Some(eq_pos) = rest.iter().position(|&c| c == '=') {
        let promo = match *rest.get(eq_pos + 1)? {
            'Q' => PieceType::Queen,
            'R' => PieceType::Rook,
            'B' => PieceType::Bishop,
            'N' => PieceType::Knight,
            _ => return None,
        };
        (&rest[..eq_pos], Some(promo))
    } else {
        (&rest[..], None)
    };

    // The last two characters are the destination square (file + rank)
    if rest.len() < 2 {
        return None;
    }
    let dest_file = rest[rest.len() - 2];
    let dest_rank = rest[rest.len() - 1];
    if !dest_file.is_ascii_lowercase() || !dest_rank.is_ascii_digit() {
        return None;
    }
    let to_file = (dest_file as u8).wrapping_sub(b'a');
    let to_rank = (dest_rank as u8).wrapping_sub(b'1');
    if to_file >= 8 || to_rank >= 8 {
        return None;
    }
    let to_sq = to_rank * 8 + to_file;

    // Everything before the destination is disambiguation (file, rank, or both)
    let disambig = &rest[..rest.len() - 2];
    let disambig_file: Option<u8> = disambig.iter()
        .find(|c| c.is_ascii_lowercase())
        .map(|&c| (c as u8) - b'a');
    let disambig_rank: Option<u8> = disambig.iter()
        .find(|c| c.is_ascii_digit())
        .map(|&c| (c as u8) - b'1');

    // Find the unique legal move matching all constraints
    let side = board.get_side_to_move();
    legal_moves.into_iter().find(|m| {
        m.to == to_sq
            && board.piece_at(m.from) == piece
            && (board.get_color_bb(side) & (1u64 << m.from)) != 0
            && m.promotion == promotion
            && disambig_file.map_or(true, |f| get_file(m.from) == f)
            && disambig_rank.map_or(true, |r| get_rank(m.from) == r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_algebraic_conversion() {
        assert_eq!(algebraic_to_square("a1"), Some(0));
        assert_eq!(algebraic_to_square("h1"), Some(7));
        assert_eq!(algebraic_to_square("a8"), Some(56));
        assert_eq!(algebraic_to_square("h8"), Some(63));
        assert_eq!(algebraic_to_square("e4"), Some(28));

        assert_eq!(square_to_algebraic(0), "a1");
        assert_eq!(square_to_algebraic(7), "h1");
        assert_eq!(square_to_algebraic(56), "a8");
        assert_eq!(square_to_algebraic(63), "h8");
        assert_eq!(square_to_algebraic(28), "e4");
    }

    #[test]
    fn test_parse_move() {
        let uci = Uci::new();

        let mv = uci.parse_move("e2e4").unwrap();
        assert_eq!(mv.from, 12); // e2
        assert_eq!(mv.to, 28); // e4
        assert!(mv.promotion.is_none());

        let mv = uci.parse_move("e7e8q").unwrap();
        assert_eq!(mv.from, 52); // e7
        assert_eq!(mv.to, 60); // e8
        assert_eq!(mv.promotion, Some(crate::board::PieceType::Queen));
    }

    #[test]
    fn test_position_startpos() {
        let mut uci = Uci::new();
        uci.handle_command("position startpos");
        // Board should be at starting position
        assert_eq!(uci.board.get_side_to_move(), crate::board::Color::White);
    }

    #[test]
    fn test_position_startpos_moves() {
        let mut uci = Uci::new();
        uci.handle_command("position startpos moves e2e4 e7e5");
        // After 1.e4 e5, it should be white's turn
        assert_eq!(uci.board.get_side_to_move(), crate::board::Color::White);
    }

    #[test]
    fn test_position_fen() {
        let mut uci = Uci::new();
        uci.handle_command("position fen rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        assert_eq!(uci.board.get_side_to_move(), crate::board::Color::Black);
    }

    #[test]
    fn test_algebraic_to_uci_pawn_moves() {
        let mut board = Board::startpos();
        assert_eq!(algebraic_to_uci(&mut board, "e4"), Some("e2e4".to_string()));
        assert_eq!(algebraic_to_uci(&mut board, "e3"), Some("e2e3".to_string()));
        assert_eq!(algebraic_to_uci(&mut board, "d4"), Some("d2d4".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_piece_moves() {
        let mut board = Board::startpos();
        assert_eq!(algebraic_to_uci(&mut board, "Nf3"), Some("g1f3".to_string()));
        assert_eq!(algebraic_to_uci(&mut board, "Nc3"), Some("b1c3".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_captures() {
        // Position after 1.e4 d5
        let mut board = Board::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2").unwrap();
        assert_eq!(algebraic_to_uci(&mut board, "exd5"), Some("e4d5".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_castling() {
        // Position where white can castle kingside
        let mut board = Board::from_fen("r1bqk2r/ppppbppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4").unwrap();
        assert_eq!(algebraic_to_uci(&mut board, "O-O"), Some("e1g1".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_promotion() {
        // White pawn on e7, can promote
        let mut board = Board::from_fen("3k4/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(algebraic_to_uci(&mut board, "e8=Q"), Some("e7e8q".to_string()));
        assert_eq!(algebraic_to_uci(&mut board, "e8=N"), Some("e7e8n".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_with_check_suffix() {
        let mut board = Board::startpos();
        // "Nf3+" should work the same as "Nf3" (strip the +)
        assert_eq!(algebraic_to_uci(&mut board, "Nf3+"), Some("g1f3".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_disambiguation() {
        // Two rooks that can both reach d1: need file disambiguation
        let mut board = Board::from_fen("3k4/8/8/8/8/8/4K3/R4R2 w - - 0 1").unwrap();
        assert_eq!(algebraic_to_uci(&mut board, "Rad1"), Some("a1d1".to_string()));
        assert_eq!(algebraic_to_uci(&mut board, "Rfd1"), Some("f1d1".to_string()));
    }

    #[test]
    fn test_algebraic_to_uci_invalid() {
        let mut board = Board::startpos();
        assert_eq!(algebraic_to_uci(&mut board, ""), None);
        assert_eq!(algebraic_to_uci(&mut board, "Zf3"), None);
        // Illegal move from start position
        assert_eq!(algebraic_to_uci(&mut board, "e5"), None);
    }
}
