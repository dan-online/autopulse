pub fn sify<T>(vec: &[T]) -> String {
    if vec.len() > 1 {
        "s".to_string()
    } else {
        "".to_string()
    }
}
