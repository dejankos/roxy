use std::fmt;
use std::fmt::{Display, Formatter};

use std::sync::Arc;

use actix_web::middleware::Logger;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, ResponseError};
use anyhow::anyhow;

use serde::Serialize;

use structopt::StructOpt;

use core::init_logger;
use core::Balancer;
use core::Configuration;
use core::FileWatcher;
use core::Proxy;

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

#[derive(Debug, Serialize)]
pub struct ErrWrapper {
    pub msg: String,
}

impl From<anyhow::Error> for ErrWrapper {
    fn from(err: anyhow::Error) -> ErrWrapper {
        let msg = err.to_string();
        ErrWrapper { msg }
    }
}

impl Display for ErrWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.msg)
    }
}

impl ResponseError for ErrWrapper {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().json(self)
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
    std::env::set_var("RUST_LOG", "actix_web=debug");
    std::env::set_var("RUST_BACKTRACE", "1");

    let cli_cfg = CliCfg::from_args();
    let configuration = Arc::new(Configuration::new(&cli_cfg.proxy_config_path)?);
    let service_config = configuration.service_config();
    init_logger(service_config.log_path, service_config.dev_mode);

    let watcher = FileWatcher::new(&cli_cfg.proxy_config_path);
    watcher.register_listener(Box::new(configuration.clone()));
    watcher.watch_file_changes()?;

    let proxy = Proxy::new(Balancer::new(configuration.clone()))?;
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
