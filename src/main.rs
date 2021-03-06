mod config;
mod file_watcher;
mod utils;

use crate::config::Configuration;
use crate::file_watcher::FileWatcher;
use actix_web::body::Body;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use log::LevelFilter;
use simplelog::{Config, TermLogger, TerminalMode};

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

    let mut watcher = FileWatcher::new("config/proxy.yaml");
    watcher.register_listener(Box::new(configuration));
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
