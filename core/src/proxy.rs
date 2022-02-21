use std::convert::TryFrom;
use std::rc::Rc;
use std::time::Duration;

use actix_web::client::{Client, ClientRequest};
use actix_web::dev::{HttpResponseBuilder, RequestHead};
use actix_web::http::Uri;
use actix_web::web::Bytes;
use actix_web::{web, HttpRequest, HttpResponse};
use anyhow::anyhow;
use anyhow::Result;

use log::info;

use url::Url;
use cache::{ExpiringCache, ResponseCache};

use crate::balancer::{Balancer, Instance};
use crate::http_utils::{get_host, Cacheable, Headers, XFF_HEADER_NAME};

// const HTTPS_SCHEME: &str = "https";

pub struct Proxy {
    balancer: Balancer,
    res_cache: ResponseCache
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
        let res_cache = ResponseCache::with_capacity(10_000);
        Ok(Proxy {
            balancer,
            res_cache,
        })
    }

    // pub async fn proxy(&self, req: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
    pub async fn proxy(&self, req: HttpRequest, body: web::Bytes) -> Result<()> {
        let key = Self::build_cache_key(&req);
        let req_cacheable = req.is_cacheable();
        if req_cacheable {
            if let Some(_res) = self.res_cache.get(key) {
                // return Ok(res);
            }
        }

        let instance = self.balancer.balance(&req).await?;
        let (mut resp_builder, bytes) = Self::send(instance, req.head(), &req, body).await?;
        let _res = resp_builder.body(bytes.clone()); // fixme conditional clone bytes from res body
        if req_cacheable {
            // self.res_cache.put(&key, &res, bytes);
        }
        Ok(())
        // Ok(res)
    }

    async fn send(
        instance: Instance,
        req_head: &RequestHead,
        req: &HttpRequest,
        body: web::Bytes,
    ) -> Result<(HttpResponseBuilder, Bytes)> {
        let proxy_uri = Self::create_proxy_uri(instance.url, req.path(), req.query_string())?;

        info!("proxying to {}", &proxy_uri);
        let mut response = Self::create_http_client(instance.timeout)
            .request_from(proxy_uri, req_head)
            .append_proxy_headers(req)
            .clear_headers()
            .send_body(body)
            .await
            .map_err(|e| anyhow!("http proxy error {}", e))?;

        let resp_builder = HttpResponse::build(response.status());
        let bytes = response.body().await?;

        Ok((resp_builder, bytes))
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

    fn build_cache_key(req: &HttpRequest) -> Rc<str> {
        Rc::from(req.uri().to_string().as_str())
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
