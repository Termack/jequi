use tokio::io::{AsyncRead, AsyncWrite};

use jequi::{HttpConn, Response};

use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
};

#[link(name = "jequi_go")]
extern "C" {
    pub fn HandleResponse(resp: *mut Response);
}

pub fn handle_static_files<'a, T: AsyncRead + AsyncWrite + Unpin>(
    http: &mut HttpConn<'a, T>,
    path: &str,
) {
    let root = Path::new(path);

    if !root.exists() {
        http.response.status = 404;
        return;
    }

    let uri = http.request.uri.trim_start_matches("/");

    let mut final_path = PathBuf::new();
    for p in Path::new(uri) {
        if p == ".." {
            final_path.pop();
        } else {
            final_path.push(p)
        }
    }

    if final_path == PathBuf::new() {
        final_path.push("index.html")
    }

    final_path = root.join(final_path);

    let mut f = match File::open(final_path) {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            http.response.status = 403;
            return;
        }
        Err(_) => {
            http.response.status = 404;
            return;
        }
    };

    match f.read(&mut http.response.body_buffer) {
        Ok(n) => http.response.body_length = n,
        Err(_) => {
            http.response.status = 404;
            http.response.body_length = 0;
            return;
        }
    }

    http.response.status = 200;
}

#[cfg(test)]
mod tests {

    static TEST_PATH: &str = "test/";

    use std::{io::Cursor, path::Path, fs::{File, self}, os::unix::prelude::PermissionsExt};

    use jequi::{HttpConn, RawStream};

    #[tokio::test]
    async fn handle_static_files_test() {
        let mut buf = [0; 35];
        let mut http = HttpConn::new(
            RawStream::Normal(Cursor::new(vec![])),
            &mut [0; 0],
            &mut buf,
        )
        .await;

        // Normal test
        http.request.uri = "/file".to_string();

        crate::handle_static_files(&mut http, TEST_PATH);

        assert_eq!(http.response.status, 200);
        assert_eq!(
            &http.response.body_buffer[..http.response.body_length],
            b"hello"
        );

        // lfi test
        http.request.uri = "/file/./../../file".to_string();
        http.response.body_length = 0;

        crate::handle_static_files(&mut http, TEST_PATH);

        assert_eq!(http.response.status, 200);
        assert_eq!(
            &http.response.body_buffer[..http.response.body_length],
            b"hello"
        );

        // Forbidden test
        let path = format!("{}noperm",TEST_PATH);
        let path = Path::new(&path);

        if !path.exists() {
            File::create(&path).unwrap();
        }

        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();

        http.request.uri = "/noperm".to_string();
        http.response.body_length = 0;

        crate::handle_static_files(&mut http, TEST_PATH);

        assert_eq!(http.response.status, 403);
        assert_eq!(&http.response.body_buffer[..http.response.body_length], b"");

        // Notfound test
        http.request.uri = "/notfound".to_string();
        http.response.body_length = 0;

        crate::handle_static_files(&mut http, TEST_PATH);

        assert_eq!(http.response.status, 404);
        assert_eq!(&http.response.body_buffer[..http.response.body_length], b"");
    }
}
