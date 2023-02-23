use std::{str::FromStr, time::Duration};

use app::prepare::MediaProcessor;

use super::argparse;

pub(crate) async fn decrypt_media_file(args: &argparse::DecryptMediaFile) -> anyhow::Result<()> {
    let mut f = tokio::io::BufReader::new(tokio::fs::File::open(args.input.as_path()).await?);
    let key = args
        .key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("must provide key"))?;
    let key_data = lucile_core::encryption_config::KeyData::from_str(key)?;
    let mut plain_reader = app::encryption::decryptor(&key_data, &mut f).await?;

    let mut of = tokio::fs::File::create(args.output.as_path()).await?;
    tokio::io::copy(&mut plain_reader, &mut of).await?;
    Ok(())
}
pub(crate) async fn split_media_file(args: &argparse::SplitMediaFile) -> anyhow::Result<()> {
    let ffmpeg = app::ffmpeg::FFMpegBinary::default();
    if args.processor {
        let split_buider = app::prepare::MediaSplittingStrategy::new(
            ffmpeg,
            Duration::from_secs_f32(args.duration),
            if args.encrypt {
                app::prepare::Encryption::EasyAes
            } else {
                app::prepare::Encryption::None
            },
            &args.output,
        )?;
        let split_task = split_buider.split_task(args.input.as_path());
        let outcome = split_task.process().await?;
        println!("{:#?}", outcome);
        return Ok(());
    }

    if args.encrypt {
        anyhow::bail!("can not encrypt without processor");
    }

    let splitter = app::ffmpeg::split::FFMpegMediaSplit::new_with_output(
        &ffmpeg,
        &args.input,
        Duration::from_secs_f32(args.duration),
        &args.output,
    )?;
    log::info!("{:#?}", splitter);
    let outcome = splitter.run().await?;
    println!("{:#?}", outcome);
    Ok(())
}
