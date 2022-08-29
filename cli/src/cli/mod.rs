pub mod argparse;
mod cli_select;
mod helpers;
mod media_intake;
mod corpus {
    use database::Database;

    use crate::cli::helpers;

    use super::argparse;

    pub(crate) async fn create_new_corpus(args: &argparse::CorpusNewOpts) -> anyhow::Result<()> {
        let db = helpers::get_database(&args.db).await?;
        log::info!("creating new corpus with name: {:?}", args.name);
        let id = db.add_corpus(&args.name).await?;
        log::info!("inserted `{}` with id={}", args.name, id);
        Ok(())
    }

    pub(crate) async fn list_all_corpus(args: &argparse::CorpusListOpts) -> anyhow::Result<()> {
        let db = helpers::get_database(&args.db).await?;
        let corpus = db.list_corpus().await?;
        for c in corpus {
            println!("{:?}", c);
        }
        Ok(())
    }
}

pub async fn run_cli(args: &argparse::CliOpts) -> anyhow::Result<()> {
    match &args.subcmd {
        argparse::SubCommand::Corpus(sub) => match sub {
            argparse::CorpusCommand::New(opts) => corpus::create_new_corpus(opts).await,
            argparse::CorpusCommand::List(opts) => corpus::list_all_corpus(opts).await
            
        },
    }
}
