use std::future::Future;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use http_body_util::{BodyExt, Collected, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::error::ReluaxError;
use crate::luax::{table_to_html, table_to_json};
use color_eyre::Result;
use rlua::Lua;

pub struct Server {
    port: u16,
    state: State,
}

#[derive(Clone)]
struct State {
    lua: Arc<Mutex<Lua>>,
    public_dir: Option<PathBuf>,
}

impl Server {
    pub async fn serve(lua: Lua, port: u16, public_dir: Option<PathBuf>) -> Result<()> {
        let state = State {
            lua: Arc::new(Mutex::new(lua)),
            public_dir,
        };
        let server = Self { port, state };
        server.start().await
    }

    async fn start(self) -> Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let listener = TcpListener::bind(addr).await?;

        let state = self.state;

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let http = http1::Builder::new();
            let state = state.clone();

            tokio::task::spawn(async move {
                if let Err(err) = http.serve_connection(io, state).await {
                    println!("Failed to serve connection: {:?}", err);
                }
            });
        }
    }
}

fn mk_response(status: StatusCode, s: String) -> Result<Response<Full<Bytes>>> {
    Ok(Response::builder()
        .status(status)
        .body(Full::new(Bytes::from(s)))?)
}

fn mk_file_response(path: PathBuf) -> Result<Response<Full<Bytes>>> {
    let ext = path.extension().unwrap().to_str().unwrap();

    let mime = match ext {
        "css" => "text/css",
        "js" => "text/javascript",
        "html" => "text/html",
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "text/plain",
    };

    let bytes = std::fs::read(path)?;

    Ok(Response::builder()
        .header("Content-Type", mime)
        .body(Full::new(Bytes::from(bytes)))?)
}

fn decode_luax_response(status: StatusCode, t: rlua::Table) -> Result<Response<Full<Bytes>>> {
    if t.contains_key("type")? {
        let ty: String = t.get("type")?;
        let mime_type: Option<String> = t.get("mime_type")?;

        match ty.as_str() {
            "html" => {
                let html: rlua::Table = t.get("value")?;
                let mut buf = Vec::new();
                table_to_html(html, &mut buf)?;
                Ok(Response::builder()
                    .header("Content-Type", mime_type.unwrap_or("text/html".to_string()))
                    .status(status)
                    .body(Full::new(Bytes::from(buf)))?)
            }
            "json" => {
                let json: rlua::Table = t.get("value")?;
                let mut buf = Vec::new();
                table_to_json(json, &mut buf)?;

                Ok(Response::builder()
                    .header(
                        "Content-Type",
                        mime_type.unwrap_or("application/json".to_string()),
                    )
                    .status(status)
                    .body(Full::new(Bytes::from(buf)))?)
            }
            "html-page" => {
                let html: rlua::Table = t.get("value")?;
                let mut buf = Vec::new();
                writeln!(&mut buf, "<!DOCTYPE html>")?;
                table_to_html(html, &mut buf)?;
                Ok(Response::builder()
                    .header("Content-Type", mime_type.unwrap_or("text/html".to_string()))
                    .status(status)
                    .body(Full::new(Bytes::from(buf)))?)
            }
            _ => Err(ReluaxError::Server("Unknown response type".to_string()).into()),
        }
    } else {
        let mut buf = Vec::new();
        writeln!(&mut buf, "<!DOCTYPE html>")?;
        table_to_html(t, &mut buf)?;
        Ok(Response::builder()
            .header("Content-Type", "text/html")
            .status(status)
            .body(Full::new(Bytes::from(buf)))?)
    }
}

impl Service<Request<Incoming>> for State {
    type Response = Response<Full<Bytes>>;
    type Error = color_eyre::Report;
    type Future =
        Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let path = req.uri().path().to_string();
        let method = req.method().clone();
        let lua = self.lua.clone();
        let public_dir = self.public_dir.clone();
        Box::pin(async {
            let body = req.into_body().collect().await?;

            Self::serve(lua, public_dir, path, method, body)
        })
    }
}

impl State {
    fn serve(
        lua: Arc<Mutex<Lua>>,
        public_dir: Option<PathBuf>,
        path: String,
        method: Method,
        body: Collected<Bytes>,
    ) -> Result<Response<Full<Bytes>>> {
        let lua = lua.lock().unwrap();

        let res = lua.context(|ctx| -> Result<Response<Full<Bytes>>> {
            let manifest: rlua::Result<rlua::Table> = ctx.load("require('reluax')").eval();

            let manifest = match manifest {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Internal lua error: {}", e);

                    return mk_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal lua error".to_string(),
                    );
                }
            };

            let route: rlua::Function = manifest.get("route")?;

            let method = method.as_str();
            let body: Vec<u8> = body.to_bytes().to_vec();

            let res: rlua::Result<(rlua::Integer, rlua::Value)> =
                route.call((path.clone(), method, body));

            let res = match res {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Internal lua error: {}", e);

                    return mk_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".to_string(),
                    );
                }
            };

            let status =
                StatusCode::from_u16(res.0 as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            if status == StatusCode::NOT_FOUND && public_dir.is_some() {
                // try to serve a static file
                let path = public_dir
                    .clone()
                    .unwrap()
                    .join(path.trim_start_matches('/'));

                if path.is_file() {
                    return mk_file_response(path);
                }
            }

            match res.1 {
                rlua::Value::String(s) => mk_response(status, s.to_str()?.to_string()),
                rlua::Value::Table(t) => decode_luax_response(status, t),
                rlua::Value::Nil => Err(ReluaxError::Server("No route found".to_string()).into()),
                rlua::Value::Error(e) => Err(ReluaxError::Lua(e).into()),
                _ => Err(ReluaxError::Server("Route returned invalid type".to_string()).into()),
            }
        })?;

        Ok(res)
    }
}
