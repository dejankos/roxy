use std::convert::TryFrom;
use std::sync::Arc;

use std::time::{Duration, Instant};

use actix_web::client::{Client, ClientRequest};
use actix_web::dev::{HttpResponseBuilder, RequestHead};
use actix_web::http::{StatusCode, Uri};
use actix_web::web::Bytes;
use actix_web::{HttpRequest, HttpResponse};
use anyhow::anyhow;
use anyhow::Result;
use log::debug;
use url::Url;

use cache::{CachedResponse, ResponseCache};

use crate::balancer::{Balancer, Instance};
use crate::http_utils::{get_host, Cacheable, Headers, XFF_HEADER_NAME};
use crate::task::spawn;

// const HTTPS_SCHEME: &str = "https";

pub struct Proxy {
    balancer: Balancer,
    res_cache: Arc<ResponseCache>,
}

trait ProxyHeaders {
    fn append_proxy_headers(self, req_from: &HttpRequest) -> Self;

    fn clear_headers(self) -> Self;
}

impl ProxyHeaders for ClientRequest {
    fn append_proxy_headers(self, req_from: &HttpRequest) -> Self {
        let headers = req_from.headers();
        let xff = headers.xff();
        let mut xff_value = String::new();
        if let Some(xff) = xff {
            xff_value.push_str(xff);
            xff_value.push_str(", ");
        }
        xff_value.push_str(get_host(req_from).as_str());

        self.set_header(XFF_HEADER_NAME, xff_value)
    }

    fn clear_headers(mut self) -> Self {
        self.headers_mut().remove("Connection");
        self
    }
}

impl Proxy {
    pub fn new(balancer: Balancer) -> Result<Self> {
        let res_cache = Arc::new(ResponseCache::with_capacity(10_000));
        let proxy = Proxy {
            balancer,
            res_cache,
        };
        proxy.run_expire();
        Ok(proxy)
    }

    pub async fn proxy(&self, req: HttpRequest, body: Bytes) -> Result<HttpResponse> {
        let key = Self::build_cache_key(&req);
        let req_cacheable = req.is_cacheable();
        if req_cacheable {
            if let Some(res) = self.cache_read(key.clone()) {
                return Ok(res);
            }
        }

        let instance = self.balancer.balance(&req).await?;
        let (mut resp_builder, bytes) = Self::send(instance, req.head(), &req, body).await?;
        let res = resp_builder.body(bytes.clone());
        if req_cacheable {
            self.cache_write(key, &res, bytes);
        }
        Ok(res)
    }

    fn cache_read(&self, key: Arc<str>) -> Option<HttpResponse> {
        if let Some(res) = self.res_cache.get(key) {
            if res.expired() {
                None
            } else {
                let mut response = HttpResponse::build(res.status_code).body(res.body);
                *response.headers_mut() = res.headers;
                Some(response)
            }
        } else {
            None
        }
    }

    fn cache_write(&self, key: Arc<str>, res: &HttpResponse, body: Bytes) {
        if res.status() != StatusCode::OK {
            return;
        }
        let max_age = res.headers().max_age();
        if max_age.is_none() {
            return;
        }

        let status_code = res.status();
        let headers = res.headers().clone();
        let ttl = Instant::now() + Duration::from_secs(max_age.unwrap());

        let response = CachedResponse {
            status_code,
            body,
            headers,
            ttl,
        };

        self.res_cache.put(key, response, ttl);
    }

    async fn send(
        instance: Instance,
        req_head: &RequestHead,
        req: &HttpRequest,
        body: Bytes,
    ) -> Result<(HttpResponseBuilder, Bytes)> {
        let proxy_uri = Self::create_proxy_uri(instance.url, req.path(), req.query_string())?;

        debug!("proxying to {}", &proxy_uri);
        let mut response = Self::create_http_client(instance.timeout)
            .request_from(proxy_uri, req_head)
            .append_proxy_headers(req)
            .clear_headers()
            .send_body(body)
            .await
            .map_err(|e| anyhow!("http proxy error {:?}", e))?;

        let resp_builder = HttpResponse::build(response.status());
        let bytes = response.body().await?;

        Ok((resp_builder, bytes))
    }

    fn run_expire(&self) {
        let cache = self.res_cache.clone();
        spawn(
            move || loop {
                cache.expire_head();
            },
            "expire-items-thread".into(),
        )
        .expect("Failed to spawn expiring cache thread");
    }

    fn create_proxy_uri(url: Url, path: &str, query_string: &str) -> Result<Uri> {
        let mut url = url;
        url.set_path(format!("{}{}", &url.path()[1..], path).as_str());
        if !query_string.is_empty() {
            url.set_query(Some(query_string));
        }

        Ok(Uri::try_from(url.as_str())?)
    }

    fn create_http_client(timeout: Duration) -> Client {
        Client::builder().timeout(timeout).finish()
    }

    fn build_cache_key(req: &HttpRequest) -> Arc<str> {
        Arc::from(req.uri().to_string().as_str())
    }

    // fn create_http_client(scheme: &str, timeout: Duration) -> Result<Client> {
    //     if scheme == HTTPS_SCHEME {
    //         let ssl_connector = SslConnector::builder(SslMethod::tls())?.build();
    //         let connector = Connector::new()
    //             .ssl(ssl_connector)
    //             .timeout(timeout)
    //             .finish();
    //
    //         Ok(Client::builder()
    //             .connector(connector)
    //             .timeout(timeout)
    //             .finish())
    //     } else {
    //     Ok(Client::builder().timeout(timeout).finish())
    //     }
    // }
}
