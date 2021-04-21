use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::sync::Arc;

use actix_web::middleware::Logger;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, ResponseError};
use anyhow::anyhow;
use log::LevelFilter;
use simplelog::{ConfigBuilder, TermLogger, TerminalMode, ThreadLogMode, WriteLogger};
use structopt::StructOpt;

use crate::balancer::Balancer;
use crate::config::Configuration;
use crate::file_watcher::FileWatcher;
use crate::proxy::Proxy;

mod balancer;
mod blocking_delay_queue;
mod cache;
mod config;
mod file_watcher;
mod http_utils;
mod matcher;
mod proxy;
mod utils;

type Response<T> = Result<T, ErrWrapper>;

#[derive(StructOpt, Debug)]
pub struct CliCfg {
    #[structopt(
        short,
        long,
        help = "Proxy configuration file",
        default_value = "config/proxy.yaml"
    )]
    proxy_config_path: String,
}

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

    let cli_cfg = CliCfg::from_args();
    let configuration = Arc::new(Configuration::new(&cli_cfg.proxy_config_path)?);
    let service_config = configuration.service_config();
    init_logger(service_config.log_path, service_config.dev_mode);

    let watcher = FileWatcher::new(&cli_cfg.proxy_config_path);
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
    .bind(format!("{}:{}", service_config.ip, service_config.port))?
    .workers(service_config.workers)
    .shutdown_timeout(10)
    .run()
    .await
    .map_err(|e| anyhow!("Startup failed {}", e))
}

fn init_logger(log_path: Option<String>, dev_mode: bool) {
    let cfg = ConfigBuilder::new()
        .set_thread_mode(ThreadLogMode::Both)
        .build();

    if log_path.is_none() || dev_mode {
        TermLogger::init(LevelFilter::Info, cfg, TerminalMode::Mixed)
            .expect("Failed to init term logger");
    } else {
        let log_file =
            File::create(format!("{}/roxy.log", log_path.unwrap())).expect("Can't create log file");
        WriteLogger::init(LevelFilter::Info, cfg, log_file).expect("Failed to init file logger")
    }
}
