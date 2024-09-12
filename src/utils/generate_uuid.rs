pub fn generate_uuid() -> String {
    let uuid = uuid::Uuid::new_v4();
    uuid.to_string()
}
