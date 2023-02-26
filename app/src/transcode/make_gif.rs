use super::{MakeGifRequest, RequestError};
use crate::{
    app::LucileApp,
    ffmpeg::gif::{FFMpegCmdAsyncResult, FFMpegGifTranscoder, GifSettings},
    LucileAppError,
};

pub async fn handle_make_gif_request(
    app: &LucileApp,
    request: &MakeGifRequest,
) -> Result<FFMpegCmdAsyncResult, LucileAppError> {
    if request.segments.len() != 1 {
        return Err(RequestError::Invalid("gifs must be exactly `1` segment".to_string()).into());
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

    let transcoder = FFMpegGifTranscoder::build_cmd(app.ffmpeg(), clip_subs, &settings)
        .await
        .map_err(RequestError::GifTranscodeError)?;
    let res = transcoder
        .launch(input)
        .await
        .map_err(RequestError::GifTranscodeError)?;

    Ok(res)
}
