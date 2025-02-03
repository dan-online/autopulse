#[cfg(test)]
mod tests {
    use crate::generate_uuid::generate_uuid;

    #[test]
    fn test_generate_uuid() {
        let uuid = generate_uuid();
        assert_eq!(uuid.len(), 36);
    }
}
