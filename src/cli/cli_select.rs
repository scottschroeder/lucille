use crate::{
    content::Content,
    service::{search::SearchResponse, transcode::ClipIdentifier},
    srt::CleanSubs,
};
use anyhow::{Context, Result};

const HIST: [&str; 6] = ["     ", "    *", "   **", "  ***", " ****", "*****"];

pub fn ask_user_for_clip(content: &Content, response: &SearchResponse) -> Result<ClipIdentifier> {
    print_top_scores(content, response);
    let input = get_user_input("make a selection: e.g. 'B 3-5'")?;
    let (index, start, end) = parse_user_selection(input.as_str())?;
    let user_clip = &response.results[index];
    Ok(ClipIdentifier {
        index: response.index,
        media_hash: user_clip.media_hash,
        start: user_clip.offset + start,
        end: user_clip.offset + end,
    })
}

pub fn print_top_scores(content: &Content, response: &SearchResponse) {
    let mut c = 'A';
    for clip in &response.results {
        let ep = content
            .episodes
            .iter()
            .find(|e| e.media_hash == clip.media_hash)
            .expect("missing episode hash");
        println!("{}) {:?}: {}", c, clip.score, ep.title);
        let base = clip.offset;
        for (offset, linescore) in clip.lines.iter().enumerate() {
            let normalized = ((5.0 * linescore.score / clip.score) + 0.5) as usize;
            let script = CleanSubs(&ep.subtitles.inner[base + offset..base + offset + 1]);
            println!("  ({:2}) [{}]- {}", offset, HIST[normalized], script);
        }
        c = ((c as u8) + 1) as char
    }
}

fn get_user_input(msg: &str) -> Result<String> {
    println!("{}", msg);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input)
}

fn parse_user_selection(s: &str) -> Result<(usize, usize, usize)> {
    let re = once_cell_regex::regex!(
        r##" *(?P<letter>[a-zA-Z]) *(?P<start>[0-9]+)(\-(?P<end>[0-9]+))?"##
    );
    let captures = re
        .captures(s)
        .ok_or_else(|| anyhow::anyhow!("could not parse user selection"))?;
    let letter = captures
        .name("letter")
        .expect("non optional regex match")
        .as_str()
        .chars()
        .next()
        .ok_or_else(|| anyhow::anyhow!("string did not contain letter?"))?;
    let start = captures
        .name("start")
        .expect("non optional regex match")
        .as_str()
        .parse::<usize>()
        .with_context(|| "unable to parse digits")?;
    let end = captures
        .name("end")
        .map(|m| {
            m.as_str()
                .parse::<usize>()
                .with_context(|| "unable to parse digits")
        })
        .transpose()?
        .unwrap_or(start);

    let user_choice_index = match letter {
        'a'..='z' => (letter as u8) - 'a' as u8,
        'A'..='Z' => (letter as u8) - 'A' as u8,
        _ => anyhow::bail!("invalid char: {:?}", letter),
    } as usize;

    Ok((user_choice_index, start, end))
}
