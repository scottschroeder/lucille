use lambda_http::{run, service_fn, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambdas::tracing::init();
    run(service_fn(lambdas::entrypoint::render)).await
}
