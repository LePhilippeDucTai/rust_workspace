use sudoku::{Board, Candidates};

struct Candidate {
    position: (usize, usize),
    value: u8,
}
enum SolverState {
    Idle,
    Computing,
    Trying(Candidate),
    Backtracking,
    Complete,
    Failed,
}

// Sert de stockage pour le backtracking
struct SolverFrame {
    board: Board,
    candidates: Candidates,
}
struct SudokuSolver {
    original_board: Board,
    board: Board,
    stack: Vec<SolverFrame>,
    current_frame: Option<SolverFrame>,
    state: SolverState,
}

impl SudokuSolver {
    fn new(&self, board: Board) -> Self {
        Self {
            original_board: board.clone(),
            board: board,
            stack: Vec::new(),
            current_frame: None,
        }
    }

    fn iter(&self) -> Board {
        let candidates = self.board.compute_candidates();
        if candidates.is_err() {}
        Board::new(self.original_board.board)
    }
}

fn main() {}
