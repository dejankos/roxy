mod config;
mod utils;
mod file_watcher;

use crate::config::Configuration;
use actix_web::body::Body;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use log::LevelFilter;
use simplelog::{Config, TermLogger, TerminalMode};
use crate::file_watcher::FileWatcher;

async fn hello(req: HttpRequest) -> HttpResponse {
    println!("hello {:?}", req);
    HttpResponse::Ok().message_body(Body::from("ello"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    std::env::set_var("RUST_BACKTRACE", "1");
    TermLogger::init(LevelFilter::Info, Config::default(), TerminalMode::Mixed).unwrap();

    let configuration = Configuration::new("config/proxy.yaml");

    let watcher = FileWatcher::new("config/proxy.yaml");
    watcher.watch();

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(web::resource("/*").to(hello))
    })
    .bind("127.0.0.1:8080")?
    .workers(4)
    .shutdown_timeout(10)
    .run()
    .await
}
