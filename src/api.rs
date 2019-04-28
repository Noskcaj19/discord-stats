use iron::prelude::*;
use iron::status;
use iron::typemap::Key;
use persistent::Read;
use std::sync::Arc;

use crate::store::StatsStore;

#[cfg(not(debug_assertions))]
const DASHBOARD_SOURCE: &str = include_str!("../web/src/index.html");
#[cfg(not(debug_assertions))]
const DASHBOARD_JS_SOURCE: &str = include_str!("../web/dist/index.js");

#[derive(Copy, Clone)]
pub struct Stats;
impl Key for Stats {
    type Value = Arc<StatsStore>;
}

pub fn total_msg_count(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_msg_count() {
        Ok(count) => Response::with((status::Ok, count.to_string())),
        Err(_) => {
            eprintln!("Error getting message count");
            Response::with((status::NoContent, "null".to_owned()))
        }
    })
}

pub fn msg_count(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_user_msg_count() {
        Ok(count) => Response::with((status::Ok, count.to_string())),
        Err(_) => {
            eprintln!("Error getting message count");
            Response::with((status::NoContent, "null".to_owned()))
        }
    })
}

pub fn msg_count_per_day(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_user_msgs_per_day() {
        Ok(count) => Response::with((status::Ok, serde_json::to_string(&count).unwrap())),
        Err(_) => {
            eprintln!("Error getting message count");
            Response::with((status::NoContent, "null".to_owned()))
        }
    })
}

pub fn total_msg_count_per_day(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_total_msgs_per_day() {
        Ok(count) => Response::with((status::Ok, serde_json::to_string(&count).unwrap())),
        Err(_) => {
            eprintln!("Error getting message count");
            Response::with((status::NoContent, r"null".to_owned()))
        }
    })
}

pub fn get_channels(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_channels() {
        Ok(ref channels) => Response::with((status::Ok, serde_json::to_string(channels).unwrap())),
        Err(_) => {
            eprintln!("Error getting channels");
            Response::with((status::InternalServerError, "[]"))
        }
    })
}

pub fn get_guilds(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_guilds() {
        Ok(ref guilds) => Response::with((status::Ok, serde_json::to_string(guilds).unwrap())),
        Err(_) => {
            eprintln!("Error getting guilds");
            Response::with((status::InternalServerError, "[]"))
        }
    })
}

#[cfg(not(debug_assertions))]
pub fn dashboard(_rq: &mut Request) -> IronResult<Response> {
    let mut resp = Response::with((status::Ok, DASHBOARD_SOURCE));
    resp.headers.set(iron::headers::ContentType::html());
    Ok(resp)
}

#[cfg(not(debug_assertions))]
pub fn dashboard_js(_rq: &mut Request) -> IronResult<Response> {
    Ok(Response::with((status::Ok, DASHBOARD_JS_SOURCE)))
}

#[cfg(debug_assertions)]
pub fn dashboard(_rq: &mut Request) -> IronResult<Response> {
    let mut resp = Response::with((
        status::Ok,
        std::fs::read_to_string("web/dist/index.html").unwrap(),
    ));
    resp.headers.set(iron::headers::ContentType::html());
    Ok(resp)
}

#[cfg(debug_assertions)]
pub fn dashboard_js(_rq: &mut Request) -> IronResult<Response> {
    let mut resp = Response::with((status::MovedPermanently, ""));
    resp.headers.set(iron::headers::Location(
        "http://localhost:1234/index.js".into(),
    ));

    Ok(resp)
}
