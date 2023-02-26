
fn import(ctx: &mut AppCtx<'_>, selection: &Path) -> anyhow::Result<()> {
    let f = std::fs::File::open(selection)
        .with_context(|| format!("unable to open file {:?} for import", selection))?;
    let packet = serde_json::from_reader(f).context("could not deserialize import packet")?;
    let lucile = ctx.lucile.clone();
    ctx.rt
        .block_on(async { import_and_index(&lucile, packet).await })
        .context("unable to run import/index in background thread")?;
    Ok(())
}
async fn import_and_index(lucile: &LucileApp, packet: CorpusExport) -> anyhow::Result<()> {
    let cid = app::import_corpus_packet(lucile, packet)
        .await
        .context("could not import packet")?;
    app::index_subtitles(lucile, cid, None)
        .await
        .context("could not index subtitles")?;
    Ok(())
}

