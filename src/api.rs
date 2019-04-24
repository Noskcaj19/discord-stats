use crate::store::StatsStore;
use iron::prelude::*;
use iron::status;
use iron::typemap::Key;
use persistent::Read;
use std::sync::Arc;

const DASHBOARD_SOURCE: &str = include_str!("../web/build/index.html");

#[derive(Copy, Clone)]
pub struct Stats;
impl Key for Stats {
    type Value = Arc<StatsStore>;
}

pub fn msg_count(req: &mut Request) -> IronResult<Response> {
    let stats = req.get::<Read<Stats>>().unwrap();

    Ok(match stats.get_msg_count() {
        Ok(count) => Response::with((status::Ok, format!(r#"{{"count": {}}}"#, count))),
        Err(_) => {
            eprintln!("Error getting message count");
            Response::with((status::NoContent, r#"{{"count": null}}"#.to_owned()))
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

pub fn dashboard(_rq: &mut Request) -> IronResult<Response> {
    let mut resp = Response::with((status::Ok, DASHBOARD_SOURCE));
    resp.headers.set(iron::headers::ContentType::html());
    Ok(resp)
}
