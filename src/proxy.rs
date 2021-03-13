use std::str::FromStr;

use actix_web::client::{Client, ClientBuilder, ClientResponse, SendRequestError};
use actix_web::dev::{Payload, PayloadStream};
use actix_web::http::Uri;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use anyhow::anyhow;
use anyhow::Result;
use log::info;

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
        let instance = self.balancer.balance(&req).await;

        info!("will proxy to {:?}", &instance);
        let x = req.path();
        let x1 = req.query_string();

        let full_address = format!("{}{}?{}", instance.address, x, x1);

        let uri = Uri::try_from(full_address)?;

        let mut client = Client::new();
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
