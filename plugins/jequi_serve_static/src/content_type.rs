use std::path::PathBuf;

pub fn get_content_type_by_path(path: &PathBuf) -> Option<&str> {
    let content_type = match path.extension()?.to_str().unwrap() {
        "js" => "text/javascript",
        "css" => "text/css",
        "csv" => "text/csv",
        "gif" => "image/gif",
        "html" => "text/html",
        "jpeg" => "image/jpeg",
        "jpg" => "image/jpeg",
        "json" => "application/json",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "mpeg" => "video/mpeg",
        "txt" => "text/plain",
        "ttf" => "font/ttf",
        "weba" => "audio/webm",
        "webm" => "video/webm",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "xhtml" => "application/xhtml+xml",
        "xml" => "application/xml",
        "zip" => "application/zip",
        _ => return None,
    };
    Some(content_type)
}
