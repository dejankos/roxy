use actix_web::client::Client;

use actix_web::http::Uri;
use actix_web::{web, HttpRequest, HttpResponse};
use anyhow::anyhow;
use anyhow::Result;
use log::debug;

use crate::balancer::Balancer;
use std::convert::TryFrom;

pub struct Proxy {
    balancer: Balancer,
}

impl Proxy {
    pub fn new(balancer: Balancer) -> Self {
        Proxy { balancer }
    }

    pub async fn proxy(&self, req: HttpRequest, body: web::Bytes) -> Result<HttpResponse> {
        let mut url = self.balancer.balance(&req).await?;
        debug!("url resolved to  {:?}", url.as_str());

        let path = format!("{}{}", url.path(), &req.path());
        debug!("req path = {}", &path);

        let mut url = url.join(path.as_str())?;
        url.set_query(Some(req.query_string()));
        debug!("url modified to {:?}", url.as_str());

        let uri = Uri::try_from(url.as_str())?;

        debug!("proxy to {:?}", &uri);

        let client = Client::new();
        let mut response = client
            .request_from(uri, req.head())
            .send_body(body)
            .await
            .map_err(|e| anyhow!("error {}", e))?;

        let mut client_resp = HttpResponse::build(response.status());
        // Remove `Connection` as per
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
        for (header_name, header_value) in response
            .headers()
            .iter()
            .filter(|(h, _)| *h != "connection")
        {
            client_resp.header(header_name.clone(), header_value.clone());
        }

        Ok(client_resp.body(response.body().await?))
    }
}
