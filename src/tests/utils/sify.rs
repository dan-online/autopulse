#[cfg(test)]
mod tests {
    use crate::utils::sify::sify;

    #[test]
    fn test_sify() {
        let vec = vec![1, 2, 3];
        assert_eq!(sify(&vec), "s".to_string());
    }

    #[test]
    fn test_sify_single() {
        let vec = vec![1];
        assert_eq!(sify(&vec), String::new());
    }
}
