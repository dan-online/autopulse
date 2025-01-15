use std::borrow::Cow;

pub fn get_url(url: &str) -> anyhow::Result<url::Url> {
    let url: Cow<str> = if url.ends_with('/') {
        Cow::Borrowed(url)
    } else {
        format!("{}/", url).into()
    };
    url::Url::parse(&url).map_err(Into::into)
}
