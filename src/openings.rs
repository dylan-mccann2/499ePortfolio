use std::path::Path;

use mongodb::Collection;
use serde::{Deserialize, Serialize};

use crate::board::Board;
use crate::uci::parse_san;

#[derive(Debug, Serialize, Deserialize)]
pub struct Opening {
    pub fen: String,
    pub name: String,
}

/// Parse a PGN move string like "1. e4 e5 2. Nf3 Nc6" into individual SAN moves.
fn parse_pgn_moves(pgn: &str) -> Vec<String> {
    pgn.split_whitespace()
        .filter(|token| !token.ends_with('.'))
        .map(|s| s.to_string())
        .collect()
}

/// Replay SAN moves on a board starting from the initial position and return
/// the resulting FEN string, or `None` if any move fails to parse.
fn pgn_to_fen(pgn: &str) -> Option<String> {
    let mut board = Board::startpos();
    let moves = parse_pgn_moves(pgn);

    for san in &moves {
        let mv = parse_san(&mut board, san)?;
        board.make_move(&mv);
    }

    Some(board.to_fen())
}

/// Read all `.tsv` files in `dir`, parse each opening's PGN into a FEN
/// position, and insert the (fen, name) pairs into a MongoDB collection.
pub async fn import_openings(dir: &Path, collection: &Collection<Opening>) -> mongodb::error::Result<()> {

    let mut openings: Vec<Opening> = Vec::new();

    for entry in std::fs::read_dir(dir).expect("failed to read openings directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("tsv") {
            continue;
        }

        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));

        for (line_num, line) in contents.lines().enumerate() {
            // Skip the header row
            if line_num == 0 {
                continue;
            }

            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                continue;
            }

            let name = cols[1];
            let pgn = cols[2];

            match pgn_to_fen(pgn) {
                Some(fen) => {
                    openings.push(Opening {
                        fen,
                        name: name.to_string(),
                    });
                }
                None => {
                    eprintln!(
                        "warning: failed to parse moves for \"{}\" in {}: {}",
                        name,
                        path.display(),
                        pgn
                    );
                }
            }
        }
    }

    if !openings.is_empty() {
        collection.insert_many(openings).await?;
    }

    println!("openings imported successfully");
    Ok(())
}

/// Look up the opening name for a given FEN position in the database.
pub async fn lookup_opening(collection: &Collection<Opening>, fen: &str) -> Option<String> {
    let filter = mongodb::bson::doc! { "fen": fen };
    collection.find_one(filter).await.ok()?.map(|o| o.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pgn_moves() {
        let moves = parse_pgn_moves("1. e4 e5 2. Nf3 Nc6");
        assert_eq!(moves, vec!["e4", "e5", "Nf3", "Nc6"]);
    }

    #[test]
    fn test_pgn_to_fen_single_move() {
        let fen = pgn_to_fen("1. e4").unwrap();
        assert_eq!(fen, "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
    }

    #[test]
    fn test_pgn_to_fen_italian() {
        let fen = pgn_to_fen("1. e4 e5 2. Nf3 Nc6 3. Bc4").unwrap();
        assert_eq!(fen, "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3");
    }

    #[test]
    fn test_pgn_to_fen_invalid() {
        assert!(pgn_to_fen("1. Zz4").is_none());
    }
}
