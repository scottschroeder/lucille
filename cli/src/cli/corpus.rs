use super::argparse;
use crate::cli::helpers;

pub(crate) async fn create_new_corpus(args: &argparse::CorpusNewOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    log::info!("creating new corpus with name: {:?}", args.name);
    let corpus = app.db.add_corpus(&args.name).await?;
    log::info!("inserted `{}` with id={}", args.name, corpus.id.unwrap());
    Ok(())
}

pub(crate) async fn list_all_corpus(args: &argparse::CorpusListOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    let corpus = app.db.list_corpus().await?;
    for c in corpus {
        println!("{:?}", c);
    }
    Ok(())
}
