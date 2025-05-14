pub fn add<T>(left: T, right: T) -> T
where
    T: std::ops::Add<Output = T>,
{
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2.0, 25.3);
        assert_eq!(result, 27.3);
    }
}
