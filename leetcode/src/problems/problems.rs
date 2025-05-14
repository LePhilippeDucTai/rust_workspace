use std::collections::HashMap;

use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use time_it_macro::time_it;

pub fn two_sums(nums: Vec<i32>, target: i32) -> (usize, usize) {
    let enums = nums.iter().enumerate();
    let inverse_image: HashMap<i32, usize> = enums.clone().map(|(i, &x)| (target - x, i)).collect();
    let result = enums
        .filter(|(_, x)| inverse_image.contains_key(x))
        .map(|(i, x)| (i, inverse_image.get(x).unwrap().to_owned()))
        .find(|(i, j)| i != j)
        .unwrap();
    result
}

pub fn two_sum(nums: Vec<i32>, target: i32) -> Vec<i32> {
    let (i, j) = two_sums(nums, target);
    let (i_32, j_32) = ((i as i32), (j as i32));
    vec![i_32, j_32]
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Box<ListNode>>,
}

impl ListNode {
    #[inline]
    fn new(val: i32) -> Self {
        ListNode { next: None, val }
    }
}
#[time_it]
pub fn merge_two_lists(
    list1: Option<Box<ListNode>>,
    list2: Option<Box<ListNode>>,
) -> Option<Box<ListNode>> {
    match (list1, list2) {
        (None, None) => None,
        (None, Some(v)) => Some(v),
        (Some(v), None) => Some(v),
        (Some(list_node_1), Some(list_node_2)) => {
            if list_node_1.val < list_node_2.val {
                let mut res = ListNode::new(list_node_1.val);
                res.next = merge_two_lists(list_node_1.next, Some(list_node_2));
                Some(Box::new(res))
            } else {
                let mut res = ListNode::new(list_node_2.val);
                res.next = merge_two_lists(Some(list_node_1), list_node_2.next);
                Some(Box::new(res))
            }
        }
    }
}

pub fn merge_k_lists(lists: Vec<Option<Box<ListNode>>>) -> Option<Box<ListNode>> {
    lists.into_iter().tree_reduce(merge_two_lists).flatten()
}

pub struct SqrtSeq {
    value: f64,
    current: f64,
}

impl SqrtSeq {
    pub fn new(value: f64) -> SqrtSeq {
        let current = value * 0.5;
        SqrtSeq { value, current }
    }
}

impl Iterator for SqrtSeq {
    type Item = f64;
    fn next(&mut self) -> Option<Self::Item> {
        let curr = 0.5 * (self.current + self.value / self.current);
        self.current = curr;
        Some(curr)
    }
}
#[time_it]
pub fn isqrt(a: u64) -> u64 {
    let m = SqrtSeq::new(a as f64);
    m.tuple_windows()
        .find(|(curr, last)| (*curr - *last).abs() < 1.)
        .unwrap()
        .1
        .floor() as u64
}
#[time_it]
pub fn compute_pi(n: u64) -> f64 {
    let dt = 1. / (n as f64);
    let pi: f64 = (0..n)
        .into_par_iter()
        .map(|i| {
            let x = (i as f64) * dt;
            f64::sqrt(1. - x * x)
        })
        .sum();
    4.0 * pi * dt
}

// pub fn combination_sum(candidates: Vec<i32>, target: i32) -> Vec<Vec<i32>> {
//     todo!()
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqrt() {
        let a = 64;
        let result = isqrt(a);
        assert_eq!(result, 8);
        assert_eq!(isqrt(144), 12);
        assert_eq!(isqrt(16), 4);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(121), 11);
        assert_eq!(isqrt(999998000001), 999999)
    }

    #[test]
    fn test_compute_pi() {
        let result = compute_pi(1_000_000);
        assert_ne!(result, 0.);
    }

    #[test]
    fn test_combination_sum() {
        let expected: Vec<Vec<i32>> = vec![vec![2, 2, 3], vec![7]];
        let candidates = vec![2, 3, 6, 7];
        let target = 7;
        let actual = combination_sum(candidates, target);
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_combination_sum_example_2() {
        let expected: Vec<Vec<i32>> = vec![vec![2, 2, 2, 2], vec![2, 3, 3], vec![3, 5]];
        let candidates = vec![2, 3, 5];
        let target = 8;
        let actual = combination_sum(candidates, target);
        assert_eq!(actual, expected)
    }

    #[test]
    fn test_combination_sum_example_3() {
        let expected: Vec<Vec<i32>> = vec![];
        let candidates = vec![2];
        let target = 1;
        let actual = combination_sum(candidates, target);
        assert_eq!(actual, expected)
    }
}
