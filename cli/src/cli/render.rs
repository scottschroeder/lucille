use app::transcode::MakeGifRequest;
use clap::Parser;

use super::argparse;

#[derive(Parser, Debug)]
pub struct RenderRequest {
    /// output gif file
    #[clap(long, default_value = "out.gif")]
    output: String,

    /// Base64 Encoded `MakeGifRequest`
    request: String,

    #[clap(flatten)]
    cfg: argparse::AppConfig,
}

impl RenderRequest {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = self.cfg.build_app().await?;

        let gif_request: MakeGifRequest = lucille_core::base64::deserialize_json(&self.request)?;

        let mut res = app::transcode::handle_make_gif_request(&app, &gif_request).await?;
        let mut output = res.output();
        let mut out_gif = tokio::fs::File::create(&self.output).await?;
        let bytes = tokio::io::copy(&mut output, &mut out_gif).await?;
        log::debug!("GIF is size: {}", bytes);
        res.wait().await?;
        Ok(())
    }
}
