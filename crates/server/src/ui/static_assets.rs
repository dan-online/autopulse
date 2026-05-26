use actix_web::{
    get,
    http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG},
    web::Path,
    HttpRequest, HttpResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

#[get("/ui/static/{path:.*}")]
pub async fn serve_static(path: Path<String>, req: HttpRequest) -> HttpResponse {
    let path = path.into_inner();
    let Some(file) = Assets::get(&path) else {
        return HttpResponse::NotFound().finish();
    };

    let etag = format!(
        "\"{:x}\"",
        u64::from_be_bytes(
            file.metadata.sha256_hash()[..8]
                .try_into()
                .unwrap_or([0; 8])
        )
    );
    if req
        .headers()
        .get("If-None-Match")
        .and_then(|h| h.to_str().ok())
        == Some(etag.as_str())
    {
        return HttpResponse::NotModified().finish();
    }

    HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, content_type_for(&path)))
        .insert_header((ETAG, etag))
        // `no-cache` = always revalidate via If-None-Match. Etag changes
        // whenever the binary is rebuilt, so revalidation is free for
        // hits and instant for misses.
        .insert_header((CACHE_CONTROL, "no-cache"))
        .body(file.data.into_owned())
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "js" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "ico" => "image/x-icon",
        "txt" | "" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
