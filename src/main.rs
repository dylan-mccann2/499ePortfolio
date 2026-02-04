use chess::board::Board;

fn main() {
    let board = Board::startpos();
    println!("Starting position: {:?}", board);
}