use std::future::Future;
use std::net::SocketAddr;
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
    lua: Arc<Mutex<Lua>>,
}

struct State {
    lua: Arc<Mutex<Lua>>,
}

impl Server {
    pub async fn serve(lua: Lua, port: u16) -> Result<()> {
        let server = Self {
            port,
            lua: Arc::new(Mutex::new(lua)),
        };
        server.start().await
    }

    async fn start(self) -> Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let state = State {
                lua: self.lua.clone(),
            };
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new().serve_connection(io, state).await {
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

impl Service<Request<Incoming>> for State {
    type Response = Response<Full<Bytes>>;
    type Error = color_eyre::Report;
    type Future =
        Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let path = req.uri().path();
        let res = self
            .serve(path)
            .map(|(st, s)| mk_response(st, s))
            .unwrap_or_else(|err| {
                println!("Error: {}", err);
                Err(err)
            });

        Box::pin(async { res })
    }
}

impl State {
    fn serve(&self, path: &str) -> Result<(StatusCode, String)> {
        let lua = self.lua.lock().unwrap();

        let res = lua.context(|ctx| -> Result<(StatusCode, String)> {
            let manifest: rlua::Table = ctx.load("require('reluax')").eval()?;

            let route: rlua::Function = manifest.get("route")?;

            let res: (rlua::Integer, rlua::Value) = route.call((path,))?;

            let status =
                StatusCode::from_u16(res.0 as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            match res.1 {
                rlua::Value::String(s) => Ok((status, s.to_str()?.to_string())),
                rlua::Value::Table(t) => {
                    let mut buf = Vec::new();
                    table_to_html(t, &mut buf)?;
                    let s = String::from_utf8(buf).unwrap();
                    Ok((status, s))
                }
                rlua::Value::Nil => Err(ReluaxError::Server("No route found".to_string()).into()),
                rlua::Value::Error(e) => Err(ReluaxError::Lua(e).into()),
                _ => Err(ReluaxError::Server("Route returned invalid type".to_string()).into()),
            }
        })?;

        Ok(res)
    }
}
