use std::{collections::HashMap, ops::Range};

use anyhow::{Context, Result};
use app::{
    app::LucileApp,
    search_manager::{ClipResult, SearchResponse},
};
use lucile_core::{clean_sub::CleanSubs, metadata::MediaMetadata, Subtitle};

const HIST: [&str; 6] = ["     ", "    *", "   **", "  ***", " ****", "*****"];

pub async fn ask_user_for_clip<'a>(
    app: &LucileApp,
    response: &'a SearchResponse,
) -> Result<(&'a ClipResult, Range<usize>)> {
    let mut content = HashMap::new();

    for clip in &response.results {
        let (_, m) = app.db.get_episode_by_id(clip.srt_id).await?;
        let subs = app.db.get_all_subs_for_srt(clip.srt_id).await?;
        content.insert(clip.srt_id, (m, subs));
    }
    print_top_scores(&content, response);
    let input = get_user_input("make a selection: e.g. 'B 3-5'")?;
    let (index, start, end) = parse_user_selection(input.as_str())?;
    let user_clip = &response.results[index];
    // let (m, sub) = content.remove(&user_clip.srt_id).unwrap();

    Ok((user_clip, (start..end)))
}

fn print_top_scores(
    content: &HashMap<i64, (MediaMetadata, Vec<Subtitle>)>,
    response: &SearchResponse,
) {
    let mut c = 'A';
    for clip in &response.results {
        let (m, subs) = &content[&clip.srt_id];
        println!("{}) {:?}: {}", c, clip.score, m.title());
        let base = clip.offset;
        for (offset, linescore) in clip.lines.iter().enumerate() {
            let normalized = ((5.0 * linescore.score / clip.score) + 0.5) as usize;
            let script = CleanSubs(&subs[base + offset..base + offset + 1]);
            println!("  ({:2}) [{}]- {}", offset, HIST[normalized], script);
        }
        c = ((c as u8) + 1) as char
    }
}

pub fn get_user_input(msg: &str) -> Result<String> {
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
