use std::future::Future;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::error::ReluaxError;
use crate::luax::table_to_html;
use color_eyre::Result;
use rlua::Lua;

pub struct Server {
    port: u16,
    state: State,
}

#[derive(Clone)]
struct State {
    lua: Arc<Mutex<Lua>>,
    public_dir: PathBuf,
}

impl Server {
    pub async fn serve(lua: Lua, port: u16, public_dir: PathBuf) -> Result<()> {
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

impl Service<Request<Incoming>> for State {
    type Response = Response<Full<Bytes>>;
    type Error = color_eyre::Report;
    type Future =
        Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let path = req.uri().path();
        let res = self.serve(path);

        Box::pin(async { res })
    }
}

impl State {
    fn serve(&self, path: &str) -> Result<Response<Full<Bytes>>> {
        let lua = self.lua.lock().unwrap();

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

            let res: rlua::Result<(rlua::Integer, rlua::Value)> = route.call((path,));

            let res = match res {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Internal lua error: {}", e);

                    return mk_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal lua error".to_string(),
                    );
                }
            };

            let status =
                StatusCode::from_u16(res.0 as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            if status == StatusCode::NOT_FOUND {
                // try to serve a static file
                let path = self.public_dir.join(path.trim_start_matches('/'));

                if path.is_file() {
                    return mk_file_response(path);
                }
            }

            match res.1 {
                rlua::Value::String(s) => mk_response(status, s.to_str()?.to_string()),
                rlua::Value::Table(t) => {
                    let mut buf = Vec::new();
                    writeln!(&mut buf, "<!DOCTYPE html>")?;
                    table_to_html(t, &mut buf)?;
                    let s = String::from_utf8(buf).unwrap();
                    mk_response(status, s)
                }
                rlua::Value::Nil => Err(ReluaxError::Server("No route found".to_string()).into()),
                rlua::Value::Error(e) => Err(ReluaxError::Lua(e).into()),
                _ => Err(ReluaxError::Server("Route returned invalid type".to_string()).into()),
            }
        })?;

        Ok(res)
    }
}
