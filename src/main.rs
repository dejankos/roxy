use actix_web::body::Body;
use actix_web::middleware::Logger;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError};
use anyhow::anyhow;
use log::LevelFilter;
use simplelog::{Config, TermLogger, TerminalMode};

use crate::balancer::Balancer;
use crate::config::Configuration;
use crate::file_watcher::FileWatcher;
use crate::proxy::Proxy;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::Arc;

mod balancer;
mod config;
mod file_watcher;
mod proxy;
mod utils;

type Response<T> = Result<T, ErrWrapper>;
#[derive(Debug)]
pub struct ErrWrapper {
    pub err: anyhow::Error,
}

impl From<anyhow::Error> for ErrWrapper {
    fn from(err: anyhow::Error) -> ErrWrapper {
        ErrWrapper { err }
    }
}

impl Display for ErrWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.err)
    }
}

impl ResponseError for ErrWrapper {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().finish()
    }
}

async fn proxy_request(
    req: HttpRequest,
    body: web::Bytes,
    proxy: web::Data<Proxy>,
) -> Response<HttpResponse> {
    Ok(proxy.proxy(req, body).await?)
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    std::env::set_var("RUST_BACKTRACE", "1");
    TermLogger::init(LevelFilter::Debug, Config::default(), TerminalMode::Mixed).unwrap(); // fixme only for dev

    let configuration = Arc::new(Configuration::new("config/proxy.yaml")?);
    let watcher = FileWatcher::new("config/proxy.yaml");
    watcher.register_listener(Box::new(configuration.clone()));
    watcher.watch_file_changes()?;

    let proxy = Proxy::new(Balancer::new(configuration.clone()));

    let data = web::Data::new(proxy);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(data.clone())
            .service(web::resource("/*").to(proxy_request))
    })
    .bind("127.0.0.1:8081")?
    .workers(4)
    .shutdown_timeout(10)
    .run()
    .await
    .map_err(|e| anyhow!("Startup failed {}", e))
}
