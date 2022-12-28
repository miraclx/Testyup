#![feature(proc_macro_hygiene, decl_macro)]
#![feature(option_result_contains)]

use std::collections::HashMap;

use tokio::sync::{oneshot, RwLock};
#[macro_use]
mod config;
use rocket::{launch, routes};

#[derive(Default)]
pub struct ServerState {
    // todo! should probably use a ttl for entries to remove them after a while
    pub oauth_handlers: RwLock<HashMap<String, oneshot::Sender<String>>>,
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/template", routes![config::create])
        .mount("/oauth", routes![config::oauth])
        .manage(ServerState::default())
}
