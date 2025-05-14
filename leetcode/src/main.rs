use leetcode::problems;
use tracing::info;

fn main() {
    tracing_subscriber::fmt::init();
    let s = problems::problems::two_sum(vec![4, 2, 8, 1, 9, 3], 11);
    info!("{s:?}");
}
