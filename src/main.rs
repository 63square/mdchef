use core::str;
use std::net::SocketAddr;
use std::path::Path;

use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, Method, StatusCode};
use tokio::fs;
use tokio::net::TcpListener;
use hyper_util::rt::TokioIo;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use pulldown_cmark::Parser;
use tokio::process::Command;

async fn locate_file(path: &str) -> Option<String> {
    if path.len() < 2 {
        return None;
    }

    let path = Path::new(path.get(1..).unwrap()).canonicalize().ok()?;

    let try_path = path.with_extension("md");
    if fs::try_exists(&try_path).await.unwrap_or(false) {
        return Some(try_path.to_string_lossy().to_string());
    }

    let try_path = path.join("index.md");
    if fs::try_exists(&try_path).await.unwrap_or(false) {
        return Some(try_path.to_string_lossy().to_string());
    }

    None
}   

async fn route(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    if req.method() != Method::GET {
        let mut bad_request = Response::new(full("Method is not allowed."));
        *bad_request.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        return Ok(bad_request);
    }
    

    let path = req.uri().path();
    if let Some(file_path) = locate_file(path).await {
        if let Ok(contents) = tokio::fs::read(file_path).await {
            if let Ok(file_c) = str::from_utf8(&contents) {
                let mut html_output = String::new();

                pulldown_cmark::html::push_html(&mut html_output, Parser::new(file_c));

                return Ok(Response::new(full(html_output)));
            }
        }
    }

    let mut not_found = Response::new(full("This file was not found"));
    *not_found.status_mut() = StatusCode::NOT_FOUND;
    return Ok(not_found);
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn open_browser(url: &str) {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .arg("/C")
            .arg(format!("start {}", url))
            .spawn()
            .expect("Failed to open browser");
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg(url)
            .spawn()
            .expect("Failed to open browser");
    } else if cfg!(target_os = "linux") {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .expect("Failed to open browser");
    } else {
        eprintln!("Unsupported platform");
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    open_browser(&(String::from("http://") + addr.to_string().as_str()));

    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);

        tokio::task::spawn(async move {
            let _ = http1::Builder::new()
                .serve_connection(io, service_fn(route))
                .await;
        });
    }
}
