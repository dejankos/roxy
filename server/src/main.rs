use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::sync::Arc;

use actix_web::middleware::Logger;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, ResponseError};
use anyhow::anyhow;
use log::LevelFilter;
use serde::Serialize;
use simplelog::{ConfigBuilder, TermLogger, TerminalMode, ThreadLogMode, WriteLogger};
use structopt::StructOpt;



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
    // proxy: web::Data<Proxy>,
) -> Response<HttpResponse> {

    // Ok(proxy.proxy(req, body).await?)
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    std::env::set_var("RUST_BACKTRACE", "1");

    let cli_cfg = CliCfg::from_args();
    Ok(())
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
