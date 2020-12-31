// # Snippet example
//
// This example shows how to return a representative snippet of
// your hit result.
// Snippet are an extracted of a target document, and returned in HTML format.
// The keyword searched by the user are highlighted with a `<b>` tag.

// ---
// Importing tantivy...
use crate::srt_loader::Episode;
use std::{collections::HashMap, path::Path};
use tantivy::{
    collector::{Collector, SegmentCollector, TopDocs},
    doc,
    query::QueryParser,
    schema::*,
    DocId, Index, Score, SegmentLocalId, SegmentReader, Snippet, SnippetGenerator,
};
use tempfile::TempDir;

pub struct Count;

impl Collector for Count {
    type Fruit = usize;

    type Child = SegmentCountCollector;

    fn for_segment(
        &self,
        slid: SegmentLocalId,
        sr: &SegmentReader,
    ) -> tantivy::Result<SegmentCountCollector> {
        log::trace!("slid: {:?}, sr: {:?}", slid, sr);
        Ok(SegmentCountCollector::default())
    }

    fn requires_scoring(&self) -> bool {
        false
    }

    fn merge_fruits(&self, segment_counts: Vec<usize>) -> tantivy::Result<usize> {
        Ok(segment_counts.into_iter().sum())
    }
}

#[derive(Default)]
pub struct SegmentCountCollector {
    count: usize,
}

impl SegmentCollector for SegmentCountCollector {
    type Fruit = usize;

    fn collect(&mut self, did: DocId, score: Score) {
        log::trace!("did: {:?}, score: {:?}", did, score);

        self.count += 1;
    }

    fn harvest(self) -> usize {
        self.count
    }
}

pub fn load_index<P: AsRef<Path>>(path: P) -> tantivy::Result<tantivy::Index> {
    let index_path = path.as_ref();
    Index::open_in_dir(&index_path)
}

pub fn build_index<P: AsRef<Path>>(path: P, eps: &[Episode]) -> tantivy::Result<tantivy::Index> {
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

    for (e_num, e) in eps.iter().enumerate() {
        for clip in e.slices(5) {
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

    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(IndexRecordOption::Basic),
        )
        .set_stored();

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

pub fn search(index: &Index, q: &str, eps: &[Episode]) -> tantivy::Result<()> {
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

    let snippet_generator = SnippetGenerator::create(&searcher, &*query, body)?;

    // println!("{:?}", top_docs);

    let mut scores = HashMap::new();

    for (score, doc_address) in top_docs {
        let doc = searcher.doc(doc_address)?;
        // let snippet = snippet_generator.snippet_from_doc(&doc);
        // let title = doc.get_first(title).unwrap().text().unwrap();
        let en = doc.get_first(episode).unwrap().u64_value() as usize;
        let cs = doc.get_first(clip_start).unwrap().u64_value() as usize;
        let ce = doc.get_first(clip_end).unwrap().u64_value() as usize;

        // println!("{}: score {}:", title, score);
        // println!("{}", snippet.fragments());
        // log::trace!("{}: [{}, {}] {}", en, cs, ce, score);

        let e_score = scores.entry(en).or_insert_with(|| {
            let e = &eps[en];
            let size = e.subs.len();
            EpisodeScore {
                name: e.title.clone(),
                inner: vec![0.0; size],
            }
        });
        e_score.add(cs, ce, score)
    }

    for es in scores.values() {
        print!("{}", es.name);
        for x in &es.inner {
            print!(", {}", x);
        }
        println!("");
    }

    Ok(())
}

struct EpisodeScore {
    name: String,
    inner: Vec<f32>,
}

impl EpisodeScore {
    fn add(&mut self, start: usize, end: usize, score: f32) {
        for s in self.inner.as_mut_slice()[start..end].iter_mut() {
            *s += score
        }
    }
}

fn highlight(snippet: Snippet) -> String {
    let mut result = String::new();
    let mut start_from = 0;

    for (start, end) in snippet.highlighted().iter().map(|h| h.bounds()) {
        result.push_str(&snippet.fragments()[start_from..start]);
        result.push_str(" --> ");
        result.push_str(&snippet.fragments()[start..end]);
        result.push_str(" <-- ");
        start_from = end;
    }

    result.push_str(&snippet.fragments()[start_from..]);
    result
}
