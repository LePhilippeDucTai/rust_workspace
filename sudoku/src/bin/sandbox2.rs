use sudoku::Board;
fn main() {
    tracing_subscriber::fmt::init();
    let board_data = [
        [9, 0, 0, 8, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 5, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 3],
        [0, 2, 0, 0, 1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0, 0, 6, 0],
        [0, 0, 0, 4, 0, 0, 0, 7, 0],
        [7, 0, 8, 6, 0, 0, 0, 0, 0],
        [7, 0, 0, 0, 3, 0, 1, 0, 0],
        [4, 0, 0, 0, 0, 0, 2, 0, 0],
    ];

    let board = Board::new(board_data);
    let candidates = board.compute_candidates();
    println!("{candidates:?}")
}
