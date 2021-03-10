use actix_web::{HttpRequest, HttpResponse};

trait Proxy {
    fn proxy(&self, req: HttpRequest) -> HttpResponse;
}
