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

enum SolverResult {
    InProgress,
    Complete,
    Failed,
}

struct SolverFrame {
    board: Board,
    candidates: Candidates,
    current_candidate: Candidate,
}

struct SudokuSolver {
    original_board: Board,
    board: Board,

    stack: Vec<SolverFrame>,
    current_frame: Option<SolverFrame>,
    solver_state: SolverState,

    current_cell: Option<(usize, usize)>,
    steps_count: u32,
    backtrack_count: u32,
}

impl SudokuSolver {
    fn new(board: Board) -> Self {
        Self {
            original_board: board.clone(),
            board: board,
            stack: Vec::new(),
            current_frame: None,
            solver_state: SolverState::Idle,
            current_cell: None,
            steps_count: 0,
            backtrack_count: 0,
        }
    }
    fn initialize_frames(&mut self) {
        // Solver
    }

    fn step(&mut self) -> SolverResult {
        match &self.solver_state {
            SolverState::Idle => return SolverResult::InProgress,
            SolverState::Complete => return SolverResult::Complete,
            SolverState::Computing => return SolverResult::InProgress,
            SolverState::Trying(candidate) => return SolverResult::InProgress,
            SolverState::Failed => return SolverResult::Failed,
            SolverState::Backtracking => return SolverResult::InProgress,
        }
    }
}

fn main() {
    println!("=== Sudoku Solver Sandbox ===\n");

    // Example 1: Simple puzzle
    println!("Testing with a simple puzzle:");
    let simple_puzzle = [
        [5, 3, 0, 0, 7, 0, 0, 0, 0],
        [6, 0, 0, 1, 9, 5, 0, 0, 0],
        [0, 9, 8, 0, 0, 0, 0, 6, 0],
        [8, 0, 0, 0, 6, 0, 0, 0, 3],
        [4, 0, 0, 8, 0, 3, 0, 0, 1],
        [7, 0, 0, 0, 2, 0, 0, 0, 6],
        [0, 6, 0, 0, 0, 0, 2, 8, 0],
        [0, 0, 0, 4, 1, 9, 0, 0, 5],
        [0, 0, 0, 0, 8, 0, 0, 7, 9],
    ];

    let board = Board::new(simple_puzzle);
    let solver = SudokuSolver::new(board);
    solver.board.pretty_print();
}
