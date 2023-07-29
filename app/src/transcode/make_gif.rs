use anyhow::Context;

use super::{MakeGifRequest, RequestError};
use crate::{
    app::LucilleApp,
    ffmpeg::gif::{FFMpegCmdAsyncResult, FFMpegGifTranscoder, GifSettings},
};

pub async fn handle_make_gif_request(
    app: &LucilleApp,
    request: &MakeGifRequest,
) -> anyhow::Result<FFMpegCmdAsyncResult> {
    if request.segments.len() != 1 {
        anyhow::bail!("not supported: gifs must contain exactly 1 segment")
    }
    let subsegment = &request.segments[0];
    let srt_uuid = subsegment.srt_uuid;
    let subs = app.db.get_all_subs_for_srt_by_uuid(srt_uuid).await?;
    let clip_subs = &subs[subsegment.sub_range.start..subsegment.sub_range.end + 1];
    let mut settings = GifSettings::default();
    let (start, end) = settings.cut_selection.content_cut_times(clip_subs);

    let target_media_view = crate::media_view::get_media_view_for_transcode(app, srt_uuid)
        .await?
        .ok_or_else(|| RequestError::NoMediaView)?;

    let (segment_start, input) =
        crate::media_view::get_surrounding_media(app, target_media_view.id, start, end).await?;
    settings.cut_selection.segment_start = Some(segment_start);

    let transcoder = FFMpegGifTranscoder::build_cmd(app.config.ffmpeg(), clip_subs, &settings)
        .await
        .context("could not build transcoder command")?;
    let res = transcoder
        .launch(input)
        .await
        .context("could not execute transcoder command")?;

    Ok(res)
}
