use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::error;
use std::fmt;
use time_it_macro::time_it;
use tracing::info;

const DIM: usize = 9;
const NULL_ELEMENT: u8 = 0;

const fn generate_candidates() -> [u8; DIM] {
    let mut arr = [0u8; DIM];
    let mut i = 0;
    while i < DIM {
        arr[i] = (i + 1) as u8;
        i += 1;
    }
    arr
}
const CANDIDATES: [u8; DIM] = generate_candidates();
const BLOCK_DIM: usize = DIM.isqrt();

type Candidates = HashMap<(usize, usize), HashSet<u8>>;

#[derive(Debug, Clone)]
pub struct InvalidSudoku;

impl fmt::Display for InvalidSudoku {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid sudoku board!")
    }
}

impl error::Error for InvalidSudoku {}

#[derive(Clone)]
pub struct Board {
    pub board: [[u8; DIM]; DIM],
}

impl Board {
    pub fn new(board: [[u8; DIM]; DIM]) -> Self {
        Board { board }
    }
    pub fn pretty_print(&self) {
        for row in self.board.iter() {
            for &cell in row.iter() {
                if cell == NULL_ELEMENT {
                    print!(". ");
                } else {
                    print!("{} ", cell);
                }
            }
            println!();
        }
    }
    #[inline]
    fn row(&self, i: usize, _: usize) -> HashSet<u8> {
        (0..DIM)
            .map(|x| self.board[i][x])
            .filter(|&x| x != 0)
            .collect()
    }
    #[inline]
    fn col(&self, _: usize, j: usize) -> HashSet<u8> {
        (0..DIM)
            .map(|x| self.board[x][j])
            .filter(|&x| x != 0)
            .collect()
    }

    fn block(&self, i: usize, j: usize) -> HashSet<u8> {
        let k = BLOCK_DIM * (i / BLOCK_DIM);
        let l = BLOCK_DIM * (j / BLOCK_DIM);
        (k..(k + BLOCK_DIM))
            .cartesian_product(l..(l + BLOCK_DIM))
            .map(|(x, y)| self.board[x][y])
            .filter(|&x| x != 0)
            .collect()
    }

    fn compute_candidates(&self) -> Result<Candidates, InvalidSudoku> {
        let mut candidates = Candidates::new();
        let nums = HashSet::from(CANDIDATES);
        let all_nulls = (0..DIM)
            .cartesian_product(0..DIM)
            .filter(|(i, j)| self.board[*i][*j] == NULL_ELEMENT);

        for (i, j) in all_nulls {
            let row = self.row(i, j);
            let col = self.col(i, j);
            let block = self.block(i, j);
            let seen = &(&row | &col) | &block;
            let union = &nums - &seen;
            if !union.is_empty() {
                candidates.insert((i, j), union);
            } else {
                return Err(InvalidSudoku);
            }
        }
        Ok(candidates)
    }

    fn with_value(self, i: usize, j: usize, value: u8) -> Board {
        let mut new_board = self;
        new_board.board[i][j] = value;
        new_board
    }
}

fn solve_sudoku(board: Board) -> Result<Board, InvalidSudoku> {
    let candidates = board.compute_candidates()?;
    let Some(((i, j), selected)) = candidates.iter().min_by_key(|(_, x)| x.len()) else {
        return Ok(board);
    };
    selected
        .iter()
        .find_map(|&v| {
            let new_board = board.clone().with_value(*i, *j, v);
            solve_sudoku(new_board).ok()
        })
        .ok_or(InvalidSudoku)
}

#[time_it]

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
        [0, 0, 0, 0, 3, 0, 1, 0, 0],
        [4, 0, 0, 0, 0, 0, 2, 0, 0],
    ];

    let board = Board::new(board_data);
    let new_board = solve(board).unwrap();
    new_board.pretty_print();
}
pub fn solve(board: Board) -> Result<Board, InvalidSudoku> {
    solve_sudoku(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_new_initializes_candidates() {
        let board_data = [
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

        let board = Board::new(board_data);

        // Check that the board is stored correctly
        assert_eq!(board.board, board_data);
        let candidates = board.compute_candidates().unwrap();
        // Check that candidates are initialized for filled cells
        for i in 0..DIM {
            for j in 0..DIM {
                if board_data[i][j] == NULL_ELEMENT {
                    assert!(candidates.contains_key(&(i, j)));
                }
            }
        }
    }

    #[test]
    fn test_solve_easy_sudoku() {
        let board_data = [
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

        let expected_solution = [
            [5, 3, 4, 6, 7, 8, 9, 1, 2],
            [6, 7, 2, 1, 9, 5, 3, 4, 8],
            [1, 9, 8, 3, 4, 2, 5, 6, 7],
            [8, 5, 9, 7, 6, 1, 4, 2, 3],
            [4, 2, 6, 8, 5, 3, 7, 9, 1],
            [7, 1, 3, 9, 2, 4, 8, 5, 6],
            [9, 6, 1, 5, 3, 7, 2, 8, 4],
            [2, 8, 7, 4, 1, 9, 6, 3, 5],
            [3, 4, 5, 2, 8, 6, 1, 7, 9],
        ];

        let board = Board::new(board_data);
        let solved = solve(board).unwrap();
        assert_eq!(solved.board, expected_solution);
    }

    #[test]
    fn test_solve_very_hard_sudoku() {
        // This is a known very hard sudoku puzzle (AI Escargot)
        let board_data = [
            [1, 0, 0, 0, 0, 7, 0, 9, 0],
            [0, 3, 0, 0, 2, 0, 0, 0, 8],
            [0, 0, 9, 6, 0, 0, 5, 0, 0],
            [0, 0, 5, 3, 0, 0, 9, 0, 0],
            [0, 1, 0, 0, 8, 0, 0, 0, 2],
            [6, 0, 0, 0, 0, 4, 0, 0, 0],
            [3, 0, 0, 0, 0, 0, 0, 1, 0],
            [0, 4, 0, 0, 0, 0, 0, 0, 7],
            [0, 0, 7, 0, 0, 0, 3, 0, 0],
        ];

        let expected_solution = [
            [1, 6, 2, 8, 5, 7, 4, 9, 3],
            [5, 3, 4, 1, 2, 9, 6, 7, 8],
            [7, 8, 9, 6, 4, 3, 5, 2, 1],
            [4, 7, 5, 3, 1, 2, 9, 8, 6],
            [9, 1, 3, 5, 8, 6, 7, 4, 2],
            [6, 2, 8, 7, 9, 4, 1, 3, 5],
            [3, 5, 6, 4, 7, 8, 2, 1, 9],
            [2, 4, 1, 9, 3, 5, 8, 6, 7],
            [8, 9, 7, 2, 6, 1, 3, 5, 4],
        ];

        let board = Board::new(board_data);
        let solved = solve(board).unwrap();
        assert_eq!(solved.board, expected_solution);
    }
}
