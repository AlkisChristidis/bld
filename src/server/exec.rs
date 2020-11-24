use crate::config::BldConfig;
use crate::server::{list, push, ws_exec, ws_monit};
use crate::term::print_info;
use crate::types::Result;
use actix::{Arbiter, System};
use actix_web::{middleware, get, web, App, HttpResponse, HttpServer, Responder};
use clap::ArgMatches;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Bld server running")
}

async fn start(host: &str, port: i64) -> Result<()> {
    print_info(&format!("starting bld server at {}:{}", host, port))?;
    std::env::set_var("RUST_LOG", "actix_server=info,actix_wev=info");
    env_logger::init();
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            .service(hello)
            .service(list)
            .service(push)
            .service(web::resource("/ws-exec/").route(web::get().to(ws_exec)))
            .service(web::resource("/ws-monit").route(web::get().to(ws_monit)))
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await?;
    Ok(())
}

pub fn sys_spawn(host: String, port: i64) {
    let system = System::new("bld-server");
    Arbiter::spawn(async move {
        let _ = start(&host, port).await;
    });
    let _ = system.run();
}

pub fn exec(matches: &ArgMatches<'_>) -> Result<()> {
    let config = BldConfig::load()?;

    let host = match matches.value_of("host") {
        Some(host) => host.to_string(),
        None => config.local.host,
    };

    let port = match matches.value_of("port") {
        Some(port) => match port.parse::<i64>() {
            Ok(port) => port,
            Err(_) => config.local.port,
        },
        None => config.local.port,
    };

    sys_spawn(host, port);
    Ok(())
}
