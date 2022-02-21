use actix_web::http::header::{HeaderMap, CACHE_CONTROL};
use actix_web::http::Method;
use actix_web::HttpRequest;

pub const XFF_HEADER_NAME: &str = "X-Forwarded-For";
const EMPTY: &str = "";
const EMPTY_STR_VEC: Vec<&str> = vec![];

pub trait Headers {
    fn get_header_value(&self, name: &str) -> Option<&str>;

    fn xff(&self) -> Option<&str>;

    fn max_age(&self) -> Option<u64>;
}

impl Headers for &HeaderMap {
    fn get_header_value(&self, name: &str) -> Option<&str> {
        if let Some(value) = self.get(name) {
            match value.to_str() {
                Ok(v) => Some(v),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn xff(&self) -> Option<&str> {
        self.get_header_value(XFF_HEADER_NAME)
    }

    fn max_age(&self) -> Option<u64> {
        let mut max_age = None;
        let mut public = false;

        self.get_all(CACHE_CONTROL)
            .into_iter()
            .flat_map(|header_value| match header_value.to_str() {
                Ok(s) => s.split(',').collect(),
                Err(_) => EMPTY_STR_VEC,
            })
            .for_each(|split| {
                if split == "public" {
                    public = true;
                }

                let kv_pair: Vec<&str> = split.split('=').collect();
                if kv_pair.len() == 2 && kv_pair[0] == "max-age" {
                    max_age = match kv_pair[1].parse() {
                        Ok(value) => Some(value),
                        Err(_) => None,
                    };
                }
            });

        if public {
            max_age
        } else {
            None
        }
    }
}

pub fn get_host(req: &HttpRequest) -> String {
    let conn_info = req.connection_info();
    let host = conn_info.host();
    if let Some(idx) = host.find(':') {
        host[..idx].into()
    } else {
        EMPTY.into()
    }
}

pub trait Cacheable {
    fn is_cacheable(&self) -> bool;
}

impl Cacheable for HttpRequest {
    fn is_cacheable(&self) -> bool {
        self.method() == Method::GET
        //todo session data ?
    }
}
