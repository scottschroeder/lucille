use crate::service::transcode::{
    ClipIdentifier, NamedFileOutput, TranscodeClient, TranscodeRequest, TranscoderService,
};
use anyhow::Result;

use crate::cli::helpers::get_storage;

fn parse_spec_shorthand(mut spec: clap::Values) -> Result<ClipIdentifier> {
    let id = spec.next().ok_or_else(|| anyhow::anyhow!("no id"))?;
    let ep = spec.next().ok_or_else(|| anyhow::anyhow!("no episode"))?;
    let start = spec.next().ok_or_else(|| anyhow::anyhow!("no start"))?;
    let end = spec.next().ok_or_else(|| anyhow::anyhow!("no end"))?;

    Ok(ClipIdentifier {
        index: uuid::Uuid::parse_str(id)?,
        episode: ep.parse::<usize>()?,
        start: start.parse::<usize>()?,
        end: end.parse::<usize>()?,
    })
}

pub fn transcode(args: &clap::ArgMatches) -> Result<()> {
    let spec = args.values_of("spec").unwrap();
    let output = args.value_of("output_gif").unwrap();
    let s = get_storage(args);

    let clip = parse_spec_shorthand(spec)?;
    let db = s.load()?;
    let gif_output = NamedFileOutput(output.to_string());
    let transcode_service = TranscoderService::new(db.id, &db.content, &db.videos, &gif_output);

    let transcode_request = TranscodeRequest { clip };
    let transcode_response = transcode_service.transcode(transcode_request)?;

    println!("{:?}", transcode_response);
    Ok(())
}
