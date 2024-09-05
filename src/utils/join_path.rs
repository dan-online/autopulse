pub fn join_path(root: &str, relative: &str) -> String {
    let root = root.strip_suffix('/').unwrap_or(root);
    let relative = relative.strip_prefix('/').unwrap_or(relative);

    format!("{root}/{relative}")
}
