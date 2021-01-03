use crate::srt_loader::IndexableEpisode;
use std::{
    collections::{BinaryHeap, HashMap},
    path::Path,
};
use tantivy::{collector::TopDocs, doc, query::QueryParser, schema::*, Index};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RankScore(pub f32);

impl Eq for RankScore {}
impl Ord for RankScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).expect("tantivy gave invalid score")
    }
}

pub fn build_index<P: AsRef<Path>>(
    path: P,
    eps: &[IndexableEpisode],
    max_window: usize,
) -> tantivy::Result<tantivy::Index> {
    let index_path = path.as_ref();

    let schema = create_schema();

    let title = get_field(&schema, SchemaField::Title);
    let body = get_field(&schema, SchemaField::Body);
    let episode = get_field(&schema, SchemaField::Episode);
    let clip_start = get_field(&schema, SchemaField::ClipStart);
    let clip_end = get_field(&schema, SchemaField::ClipEnd);

    // # Indexing documents
    let index = Index::create_in_dir(&index_path, schema)?;

    let mut index_writer = index.writer(50_000_000)?;

    for (e_num, episode_data) in eps.iter().enumerate() {
        for clip in episode_data.slices(max_window) {
            index_writer.add_document(doc!(
                title => clip.title,
                body => clip.text,
                episode => e_num as u64,
                clip_start => clip.start as u64,
                clip_end => clip.end as u64,
            ));
        }
    }
    index_writer.commit()?;
    Ok(index)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SchemaField {
    Title,
    Body,
    Episode,
    ClipStart,
    ClipEnd,
}

impl SchemaField {
    fn as_str(self) -> &'static str {
        match self {
            SchemaField::Title => "title",
            SchemaField::Body => "body",
            SchemaField::Episode => "episode",
            SchemaField::ClipStart => "clip_start",
            SchemaField::ClipEnd => "clip_end",
        }
    }
}

fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    let text_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("en_stem")
            .set_index_option(IndexRecordOption::Basic),
    );

    schema_builder.add_text_field(SchemaField::Title.as_str(), text_options.clone());
    schema_builder.add_text_field(SchemaField::Body.as_str(), text_options);
    schema_builder.add_u64_field(SchemaField::Episode.as_str(), STORED);
    schema_builder.add_u64_field(SchemaField::ClipStart.as_str(), STORED);
    schema_builder.add_u64_field(SchemaField::ClipEnd.as_str(), STORED);
    schema_builder.build()
}

fn get_field(schema: &Schema, field: SchemaField) -> Field {
    schema
        .get_field(field.as_str())
        .expect("field in enum was not in schema")
}

pub fn search(
    index: &Index,
    q: &str,
    search_window: usize,
) -> tantivy::Result<HashMap<usize, EpisodeScore>> {
    let read_schema = create_schema();

    let body = get_field(&read_schema, SchemaField::Body);
    let episode = get_field(&read_schema, SchemaField::Episode);
    let clip_start = get_field(&read_schema, SchemaField::ClipStart);
    let clip_end = get_field(&read_schema, SchemaField::ClipEnd);

    let reader = index.reader()?;
    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, vec![body]);
    let query = query_parser.parse_query(q)?;

    let top_docs = searcher.search(&query, &TopDocs::with_limit(10000))?;

    let mut scores = HashMap::new();

    for (score, doc_address) in top_docs {
        let doc = searcher.doc(doc_address)?;
        let en = doc.get_first(episode).unwrap().u64_value() as usize;
        let cs = doc.get_first(clip_start).unwrap().u64_value() as usize;
        let ce = doc.get_first(clip_end).unwrap().u64_value() as usize;

        if ce - cs > search_window {
            continue;
        }

        let e_score = scores.entry(en).or_insert_with(|| EpisodeScore {
            inner: vec![],
            episode: en,
        });
        e_score.add(cs, ce, score)
    }

    Ok(scores)
}

pub fn rank(scores: &HashMap<usize, EpisodeScore>, top: usize) -> Vec<RankedMatch> {
    let ranked = scores
        .values()
        .map(|es| {
            let matches = ClipMatches {
                data: es.inner.as_slice(),
                cursor: 0,
            };
            (es.episode, matches)
        })
        .flat_map(|(episode, matches)| {
            let ep = episode;
            matches.map(move |clip| {
                let score = *clip.scores.iter().max().unwrap();
                RankedMatch { score, ep, clip }
            })
        })
        .collect::<BinaryHeap<RankedMatch>>();
    ranked.into_iter_sorted().take(top).collect()
}

pub struct EpisodeScore {
    episode: usize,
    inner: Vec<RankScore>,
}

impl EpisodeScore {
    fn add(&mut self, start: usize, end: usize, score: f32) {
        if self.inner.len() <= end {
            let extend = 1 + end - self.inner.len();
            self.inner
                .extend(std::iter::repeat(RankScore(0.0)).take(extend))
        }
        for s in self.inner.as_mut_slice()[start..end].iter_mut() {
            s.0 += score
        }
    }
}

const MIN_SCORE: f32 = 0.5f32;

struct ClipMatches<'a> {
    data: &'a [RankScore],
    cursor: usize,
}

#[derive(PartialEq, Eq)]
pub struct RankedMatch<'a> {
    pub score: RankScore,
    pub ep: usize,
    pub clip: ClipMatch<'a>,
}

impl<'a> PartialOrd for RankedMatch<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl<'a> Ord for RankedMatch<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

#[derive(PartialEq, Eq)]
pub struct ClipMatch<'a> {
    pub index: usize,
    pub scores: &'a [RankScore],
}

impl<'a> Iterator for ClipMatches<'a> {
    type Item = ClipMatch<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.cursor < self.data.len() && self.data[self.cursor].0 < MIN_SCORE {
            self.cursor += 1
        }
        let index = self.cursor;
        while self.cursor < self.data.len() && self.data[self.cursor].0 > MIN_SCORE {
            self.cursor += 1
        }
        if self.cursor > index {
            Some(ClipMatch {
                index,
                scores: &self.data[index..self.cursor],
            })
        } else {
            None
        }
    }
}
