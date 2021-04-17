use std::convert::TryFrom;

use std::time::Duration;


use actix_web::dev::{HttpResponseBuilder, RequestHead};
use actix_web::http::Uri;
use actix_web::web::Bytes;
use actix_web::{web, HttpRequest, HttpResponse};
use anyhow::anyhow;
use anyhow::Result;
use awc::{Client, ClientRequest};
use awc::{Connector};
use log::debug;
use openssl::ssl::{SslConnector, SslMethod};
use url::Url;

use crate::balancer::Balancer;
use crate::cache::ResponseCache;
use crate::http_utils::{get_host, is_cacheable, Headers, XFF_HEADER_NAME};

const HTTPS_SCHEME: &str = "https";

pub struct Proxy {
    balancer: Balancer,
    res_cache: ResponseCache,
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
        let res_cache = ResponseCache::new(10_000)?; //todo from config
        Ok(Proxy {
            balancer,
            res_cache,
        })
    }

    pub async fn proxy(&self, req: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
        let key = ResponseCache::build_cache_key(&req);
        let req_cacheable = is_cacheable(&req);
        if req_cacheable {
            if let Some(res) = self.res_cache.get(&key) {
                let mut response = HttpResponse::build(res.status_code).body(res.body);
                *response.headers_mut() = res.headers;
                return Ok(response);
            }
        }

        let instance = self.balancer.balance(&req).await?;
        let scheme = instance.url.scheme().to_owned();
        let proxy_uri = Self::create_proxy_uri(instance.url, req.path(), req.query_string())?;

        debug!("proxying to {}", &proxy_uri);
        let (mut response, bytes) =
            Self::send(scheme, instance.timeout, proxy_uri, req.head(), &req, body).await?;

        //todo cache res

        Ok(response.body(bytes))
    }

    async fn send(
        scheme: String,
        timeout: Duration,
        proxy_uri: Uri,
        req_head: &RequestHead,
        req: &HttpRequest,
        body: web::Bytes,
    ) -> Result<(HttpResponseBuilder, Bytes)> {
        let mut response = Self::create_http_client(scheme.as_str(), timeout)?
            .request_from(proxy_uri, req_head)
            .append_proxy_headers(req)
            .clear_headers()
            .send_body(body)
            .await
            .map_err(|e| anyhow!("http proxy error {}", e))?;

        let client_resp = HttpResponse::build(response.status());
        let bytes = response.body().await?;

        Ok((client_resp, bytes))
    }

    fn create_proxy_uri(url: Url, path: &str, query_string: &str) -> Result<Uri> {
        let mut url = url;
        url.set_path(format!("{}{}", &url.path()[1..], path).as_str());
        if !query_string.is_empty() {
            url.set_query(Some(query_string));
        }

        Ok(Uri::try_from(url.as_str())?)
    }

    fn create_http_client(scheme: &str, timeout: Duration) -> Result<Client> {
        if scheme == HTTPS_SCHEME {
            let ssl_connector = SslConnector::builder(SslMethod::tls())?.build();
            let connector = Connector::new()
                .ssl(ssl_connector)
                .timeout(timeout)
                .finish();

            Ok(Client::builder()
                .connector(connector)
                .timeout(timeout)
                .finish())
        } else {
            Ok(Client::builder().timeout(timeout).finish())
        }
    }
}
