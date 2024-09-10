use super::settings::Rewrite;

// TODO: Refactor to allow for regex rewrites
pub fn rewrite_path(path: String, rewrite: &Rewrite) -> String {
    let from = rewrite.from.clone();
    let to = rewrite.to.clone();

    path.replace(&from, &to)
}
