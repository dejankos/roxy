use actix_web::body::Body;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use anyhow::anyhow;
use anyhow::Result;
use log::LevelFilter;
use simplelog::{Config, TermLogger, TerminalMode};

use crate::balancer::Balancer;
use crate::config::Configuration;
use crate::file_watcher::FileWatcher;
use crate::proxy::Proxy;

mod balancer;
mod config;
mod file_watcher;
mod proxy;
mod utils;

async fn hello(req: HttpRequest, body: web::Bytes, proxy: web::Data<Proxy>) -> HttpResponse { // return result
    println!("hello {:?}", req);
    // HttpResponse::Ok().message_body(Body::from("ello"))

    proxy.proxy(req, body).await.unwrap()
}

#[actix_web::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    std::env::set_var("RUST_BACKTRACE", "1");
    TermLogger::init(LevelFilter::Debug, Config::default(), TerminalMode::Mixed).unwrap(); // fixme only for dev

    let configuration = Configuration::new("config/proxy.yaml")?;
    // let watcher = FileWatcher::new("config/proxy.yaml");
    // watcher.register_listener(Box::new(configuration));
    // watcher.watch_file_changes()?;

    let proxy = Proxy::new(Balancer::new(configuration));

    let data = web::Data::new(proxy);

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(data.clone())
            .service(web::resource("/*").to(hello))
    })
    .bind("127.0.0.1:8081")?
    .workers(4)
    .shutdown_timeout(10)
    .run()
    .await
    .map_err(|e| anyhow!("Startup failed {}", e))
}
