// # Snippet example
//
// This example shows how to return a representative snippet of
// your hit result.
// Snippet are an extracted of a target document, and returned in HTML format.
// The keyword searched by the user are highlighted with a `<b>` tag.

// ---
// Importing tantivy...
use std::collections::HashMap;
use tantivy::collector::{Collector, SegmentCollector};
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::SegmentLocalId;
use tantivy::{collector::TopDocs, DocId, Score, SegmentReader};
use tantivy::{doc, Index, Snippet, SnippetGenerator};
use tempfile::TempDir;

use crate::srt_loader::Episode;

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

pub fn build_index(eps: &HashMap<String, String>) -> tantivy::Result<tantivy::Index> {
    // Let's create a temporary directory for the
    // sake of this example
    let index_path = TempDir::new()?;

    // # Defining the schema
    let mut schema_builder = Schema::builder();
    let title = schema_builder.add_text_field("title", TEXT | STORED);
    let body = schema_builder.add_text_field("body", TEXT | STORED);
    let schema = schema_builder.build();

    // # Indexing documents
    let index = Index::create_in_dir(&index_path, schema.clone())?;

    let mut index_writer = index.writer(50_000_000)?;

    // we'll only need one doc for this example.
    for (e_name, e_script) in eps {
        index_writer.add_document(doc!(
            title => e_name.as_str(),
            body => e_script.as_str(),
        ));
    }
    index_writer.commit()?;
    Ok(index)
}

pub fn search(q: &str, eps: &[Episode]) -> tantivy::Result<()> {
    // Let's create a temporary directory for the
    // sake of this example
    let index_path = TempDir::new()?;

    // # Defining the schema
    let mut schema_builder = Schema::builder();

    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("en_stem")
                .set_index_option(IndexRecordOption::Basic),
        )
        .set_stored();

    let title = schema_builder.add_text_field("title", text_options.clone());
    let body = schema_builder.add_text_field("body", text_options);
    let episode = schema_builder.add_u64_field("episode", STORED);
    let clip_start = schema_builder.add_u64_field("clip_start", STORED);
    let clip_end = schema_builder.add_u64_field("clip_end", STORED);
    let schema = schema_builder.build();

    // # Indexing documents
    let index = Index::create_in_dir(&index_path, schema.clone())?;

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
    // let index = build_index(eps)?;
    // let title = index.schema().get_field("title").unwrap();
    // let body = index.schema().get_field("body").unwrap();

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
    inner: Vec<f32>
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
