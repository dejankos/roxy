use std::convert::TryFrom;
use std::time::Duration;

use actix_web::http::Uri;
use actix_web::{web, HttpRequest, HttpResponse};
use anyhow::anyhow;
use anyhow::Result;
use awc::{http, Connector};
use awc::{Client, ClientRequest};
use log::debug;
use log::error;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use url::Url;

use crate::balancer::Balancer;

pub struct Proxy {
    balancer: Balancer,
}

trait ProxyHeaders {
    fn append_proxy_headers(self, req_from: &HttpRequest) -> Self;
}

trait RequestHeaders {
    fn get_header_value(&self, name: &str) -> Option<&str>;
}

impl RequestHeaders for HttpRequest {
    fn get_header_value(&self, name: &str) -> Option<&str> {
        if let Some(value) = self.headers().get(name) {
            match value.to_str() {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("Error reading header {}, e {}", name, e);
                    None
                }
            }
        } else {
            None
        }
    }
}

impl ProxyHeaders for ClientRequest {
    fn append_proxy_headers(self, req_from: &HttpRequest) -> Self {
        let xff = req_from.get_header_value("X-Forwarded-For");
        let xfh = req_from.get_header_value("X-Forwarded-Host");
        let xfp = req_from.get_header_value("X-Forwarded-Proto");

        if let Some(v) = xff {
            self.set_header(
                "X-Forwarded-For",
                format!("{}, {}", v, req_from.connection_info().host()),
            )
        } else {
            self.set_header("X-Forwarded-For", req_from.connection_info().host())
        }
    }
}

impl Proxy {
    pub fn new(balancer: Balancer) -> Self {
        Proxy { balancer }
    }

    pub async fn proxy(&self, req: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
        let instance_url = self.balancer.balance(&req).await?;
        let scheme = String::from(instance_url.scheme());
        let proxy_uri = create_proxy_uri(instance_url, req.path(), req.query_string())?;

        debug!("proxying to {}", &proxy_uri);
        let mut response = create_http_client(scheme.as_str(), Duration::from_secs(100))? // todo add to config per endpoint
            .request_from(proxy_uri, req.head())
            .append_proxy_headers(&req)
            .send_body(body)
            .await
            .map_err(|e| anyhow!("http proxy error {}", e))?;

        let mut client_resp = HttpResponse::build(response.status());
        // Remove `Connection` as per
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
        // for (header_name, header_value) in response
        //     .headers()
        //     .iter()
        //     .filter(|(h, _)| *h != "connection")
        // {
        //     client_resp.header(header_name.clone(), header_value.clone());
        // }

        Ok(client_resp.body(response.body().await?))
    }
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
    if scheme == "https" {
        let mut ssl_connector = SslConnector::builder(SslMethod::tls())?.build();
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

#[cfg(test)]
mod tests {
    use url::Url;

    use crate::proxy::create_proxy_uri;

    #[test]
    fn should_create_uri() {
        let url = Url::parse("http://localhost:8080").unwrap();
        let path = "v1/test";
        let query_string = "";

        assert_eq!(
            "http://localhost:8080/v1/test",
            create_proxy_uri(url, path, query_string).unwrap()
        );
    }

    #[test]
    fn should_create_uri_with_query_string() {
        let url = Url::parse("http://localhost:8080").unwrap();
        let path = "v1/test";
        let query_string = "a=1&b=2";

        assert_eq!(
            "http://localhost:8080/v1/test?a=1&b=2",
            create_proxy_uri(url, path, query_string).unwrap()
        );
    }

    #[test]
    fn should_resolve_scheme() {
        let url = Url::parse("http://localhost:8080").unwrap();
        assert_eq!("http", url.scheme());
        let url = Url::parse("https://localhost:8080").unwrap();
        assert_eq!("https", url.scheme());
    }
}
