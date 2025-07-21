pub struct Solution;
// https://leetcode.com/problems/candy/

impl Solution {
    pub fn candy(ratings: Vec<i32>) -> i32 {
        ratings[0]
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_1() {
        let ratings = vec![1, 0, 2];
        assert_eq!(Solution::candy(ratings), 5);
    }

    #[test]
    fn test_example_2() {
        let ratings = vec![1, 2, 2];
        assert_eq!(Solution::candy(ratings), 4);
    }

    #[test]
    fn test_all_same_ratings() {
        let ratings = vec![3, 3, 3, 3];
        assert_eq!(Solution::candy(ratings), 4);
    }

    #[test]
    fn test_strictly_increasing() {
        let ratings = vec![1, 2, 3, 4, 5];
        assert_eq!(Solution::candy(ratings), 15);
    }

    #[test]
    fn test_strictly_decreasing() {
        let ratings = vec![5, 4, 3, 2, 1];
        assert_eq!(Solution::candy(ratings), 15);
    }

    #[test]
    fn test_single_child() {
        let ratings = vec![10];
        assert_eq!(Solution::candy(ratings), 1);
    }

    #[test]
    fn test_two_children_equal() {
        let ratings = vec![2, 2];
        assert_eq!(Solution::candy(ratings), 2);
    }

    #[test]
    fn test_two_children_different() {
        let ratings = vec![1, 2];
        assert_eq!(Solution::candy(ratings), 3);
        let ratings = vec![2, 1];
        assert_eq!(Solution::candy(ratings), 3);
    }

    #[test]
    fn test_zigzag() {
        let ratings = vec![1, 3, 2, 2, 1];
        assert_eq!(Solution::candy(ratings), 7);
    }
}
