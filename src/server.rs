use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::error::ReluaxError;
use crate::{table_to_html, Result};
use rlua::Lua;

pub struct Server {
    lua: Arc<Mutex<Lua>>,
}

struct State {
    lua: Arc<Mutex<Lua>>,
}

static PORT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

impl Server {
    pub async fn serve(port: u16) -> Result<()> {
        PORT.set(port).unwrap();

        let server = Self {
            lua: Arc::new(Mutex::new(Lua::new())),
        };
        server.start().await
    }

    async fn start(self) -> Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], *PORT.get().unwrap()));
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let state = State {
                lua: self.lua.clone(),
            };
            tokio::task::spawn(async move {
                tokio::task::spawn(async move {
                    if let Err(err) = http1::Builder::new().serve_connection(io, state).await {
                        println!("Failed to serve connection: {:?}", err);
                    }
                });
            });
        }
    }
}

fn mk_response(s: String) -> Result<Response<Full<Bytes>>> {
    Ok(Response::builder().body(Full::new(Bytes::from(s))).unwrap())
}

impl Service<Request<Incoming>> for State {
    type Response = Response<Full<Bytes>>;
    type Error = color_eyre::Report;
    type Future =
        Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let path = req.uri().path();
        let res = self.serve(path).map(mk_response).unwrap_or_else(|err| {
            println!("Error: {}", err);
            Err(err)
        });

        Box::pin(async { res })
    }
}

impl State {
    fn serve(&self, path: &str) -> Result<String> {
        let lua = self.lua.lock().unwrap();

        let res = lua.context(|ctx| -> Result<String> {
            let manifest: rlua::Table = ctx.load("require('reluax')").eval()?;

            let route: rlua::Function = manifest.get("route")?;

            let res: rlua::Value = route.call((path,))?;

            match res {
                rlua::Value::String(s) => Ok(s.to_str()?.to_string()),
                rlua::Value::Table(t) => {
                    let mut buf = Vec::new();
                    table_to_html(t, &mut buf)?;
                    let s = String::from_utf8(buf).unwrap();
                    Ok(s)
                }
                rlua::Value::Nil => Err(ReluaxError::Server("No route found".to_string()).into()),
                rlua::Value::Error(e) => Err(ReluaxError::Lua(e).into()),
                _ => Err(ReluaxError::Server("Route returned invalid type".to_string()).into()),
            }
        })?;

        Ok(res)
    }
}
