#[cfg(feature = "lambda")]
pub mod tracing {
    pub fn init() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            // disable printing the name of the module in every log line.
            .with_target(false)
            // disabling time is handy because CloudWatch will add the ingestion time.
            .without_time()
            .init();
    }
}

pub mod entrypoint {
    use lambda_http::{Body, Error, Request, Response};

    use crate::render::handle_render_request;

    const EPHEMERAL_HEADER: &str = "x-ephemeral-store";

    /// This is the main body for the function.
    /// Write your code inside it.
    /// There are some code example in the following URLs:
    /// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
    pub async fn render(event: Request) -> Result<Response<Body>, Error> {
        // Extract some useful information from the request

        let make_gif_request: Result<app::transcode::MakeGifRequest, serde_json::Error> =
            serde_json::from_slice(event.body());

        let temporary = event
            .headers()
            .get(EPHEMERAL_HEADER)
            .and_then(|h| h.to_str().ok())
            .map(|s| matches!(s.to_lowercase().as_str(), "1" | "true"))
            .unwrap_or(false);

        let mut response_text = String::new();
        if let Ok(req) = &make_gif_request {
            match handle_render_request(req, temporary).await {
                Ok(s) => {
                    response_text.push_str(&s);
                }
                Err(e) => {
                    log::error!("error handling request: {:#}", e);
                }
            }
        }

        let resp = Response::builder()
            .status(200)
            .header("content-type", "text/html")
            .header(EPHEMERAL_HEADER, if temporary { "true" } else { "false" })
            .body(response_text.into())
            .map_err(Box::new)?;
        Ok(resp)
    }
}

pub(crate) mod render {
    use std::time::{Duration, SystemTime};

    use anyhow::Context;
    use app::transcode::MakeGifRequest;
    use aws_sdk_s3::types::DateTime;

    pub(crate) async fn handle_render_request(
        req: &MakeGifRequest,
        temporary: bool,
    ) -> anyhow::Result<String> {
        log::info!("lucille requst: {:?}", req);
        let app = crate::common::build_app()
            .await
            .context("could not build app")?;
        log::info!("Lucille App: {:?}", app);

        let output_bucket = app
            .config
            .output_s3_bucket()
            .ok_or_else(|| anyhow::anyhow!("no upload bucket configured"))?;
        let gif_uuid = lucille_core::uuid::Uuid::generate();

        let cfg = aws_config::from_env().load().await;
        let s3_config = aws_sdk_s3::config::Config::new(&cfg);
        let client = aws_sdk_s3::Client::from_conf(s3_config);
        let region = cfg
            .region()
            .ok_or_else(|| anyhow::anyhow!("unknown aws region"))?;

        let mut res = app::transcode::handle_make_gif_request(&app, req)
            .await
            .context("error making gif transcode plan")?;
        let mut output = res.output();

        let mut buf = Vec::new();
        tokio::io::copy(&mut output, &mut buf)
            .await
            .context("error copying ffmpeg output to destination")?;
        res.wait().await.context("error from ffmpeg command")?;

        log::info!("GIF is size: {}", buf.len());
        let key = format!("v1/{}.gif", gif_uuid);
        client
            .put_object()
            .bucket(&output_bucket)
            .key(&key)
            .content_type("image/gif")
            .body(buf.into())
            .set_expires(temporary.then(|| {
                let expire = SystemTime::now()
                    .checked_add(Duration::from_secs(60 * 60))
                    .unwrap();
                DateTime::from(expire)
            }))
            .send()
            .await
            .context("s3 put failure")?;
        let url = format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            output_bucket, region, key
        );

        Ok(url)
    }
}

pub(crate) mod common {
    use anyhow::Context;

    pub(crate) async fn build_app() -> anyhow::Result<app::app::LucilleApp> {
        let config = app::app::ConfigBuilder::new_with_user_dirs()?
            .load_environment(true)
            .build()?;

        let db_opts = config
            .database_connection_opts()
            .context("failed to create db opts")?
            .immutable();

        let mut db_builder = database::DatabaseBuider::default();
        db_builder
            .add_opts(db_opts)
            .context("database configuration")?;
        db_builder.connect().await.context("connect to db")?;
        db_builder
            .migrate()
            .await
            .context("validate db migrations")?;
        let (db, _) = db_builder.into_parts().context("db buidler into parts")?;
        let mut app = app::app::LucilleApp::new(db, config);
        app.add_s3_backend().await;
        Ok(app)
    }
}
