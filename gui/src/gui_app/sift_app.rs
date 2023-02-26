use super::{
    components::choice::{ChoiceOutcome, ChoicePane, ChoiceSelection},
    database::{Corpus, Item},
    AppCtx,
};
use crate::app::components::card::CardDisplay;
use rand::prelude::{Rng, ThreadRng};
use ranker::{Outcome, PickerPair};
use std::collections::HashMap;

struct FlippableChoice {
    original: ranker::PickerPair,
    flip: bool,
}

impl FlippableChoice {
    fn new(pair: ranker::PickerPair, flip: bool) -> FlippableChoice {
        FlippableChoice {
            original: pair,
            flip,
        }
    }

    fn new_cointoss(rng: &mut ThreadRng, pair: ranker::PickerPair) -> FlippableChoice {
        let flip = rng.gen_bool(0.5);
        FlippableChoice::new(pair, flip)
    }

    fn map_outcome(&self, outcome: ranker::Outcome) -> ranker::Outcome {
        if self.flip {
            match outcome {
                Outcome::Left => Outcome::Right,
                Outcome::Right => Outcome::Left,
                Outcome::Equal => Outcome::Equal,
            }
        } else {
            outcome
        }
    }

    fn get_pair(&self) -> ranker::PickerPair {
        if self.flip {
            PickerPair(self.original.1, self.original.0)
        } else {
            self.original
        }
    }
}

fn get_choice(rng: &mut ThreadRng, engine: &mut ranker::Ranker) -> anyhow::Result<FlippableChoice> {
    let pair = engine
        .gen_pair(rng)
        .ok_or_else(|| anyhow::anyhow!("no more selections left"))?;
    Ok(FlippableChoice::new_cointoss(rng, pair))
}

struct OrderedItems<'a, T> {
    items: &'a [Item<T>],
    order: &'a ranker::OrderedScores,
}

fn load_scores<T>(
    ctx: &mut AppCtx<'_>,
    corpus: &Corpus,
    items: &[Item<T>],
) -> anyhow::Result<ranker::ScoreDB> {
    let mut scores = ranker::ScoreDB::new(items.len());
    let lookup_id = items
        .iter()
        .enumerate()
        .map(|(idx, e)| (e.id, idx))
        .collect::<HashMap<_, _>>();

    ctx.db.load_preferences(corpus, |p| {
        let lhs = lookup_id.get(&p.lhs_id).copied();
        let rhs = lookup_id.get(&p.rhs_id).copied();
        if let Some((lhs, rhs)) = lhs.zip(rhs) {
            scores.record_outcome(lhs, rhs, p.outcome)
        }
        Ok(())
    })?;
    Ok(scores)
}

pub(crate) struct SiftApp<T> {
    corpus: Corpus,
    items: Vec<Item<T>>,
    choice: FlippableChoice,
    engine: ranker::Ranker,
}

impl<T> SiftApp<T> {
    pub fn new<V: Into<Vec<Item<T>>>>(corpus: Corpus, items: V) -> anyhow::Result<SiftApp<T>> {
        let items = items.into();
        let scores = ranker::ScoreDB::new(items.len());
        let mut engine = ranker::create_engine(scores, items.len());
        let mut rng = rand::thread_rng();
        let choice = get_choice(&mut rng, &mut engine)?;
        Ok(SiftApp {
            corpus,
            items,
            choice,
            engine,
        })
    }

    pub(crate) fn load_scores(&mut self, ctx: &mut AppCtx<'_>) -> anyhow::Result<()> {
        let scores = load_scores(ctx, &self.corpus, self.items.as_slice())?;
        self.engine = ranker::create_engine(scores, self.items.len());
        self.get_next()?;
        Ok(())
    }

    fn item_choices(&self) -> (&Item<T>, &Item<T>) {
        let PickerPair(lhs, rhs) = self.choice.get_pair();
        (&self.items[lhs], &self.items[rhs])
    }

    fn get_next(&mut self) -> anyhow::Result<()> {
        let mut rng = rand::thread_rng();
        let choice = get_choice(&mut rng, &mut self.engine)?;
        self.choice = choice;
        Ok(())
    }

    fn record_preference(&mut self, outcome: ranker::Outcome, ctx: &mut AppCtx<'_>) {
        let (lhs, rhs) = self.item_choices();
        if let Err(e) = ctx.db.save_preference(lhs.id, rhs.id, outcome) {
            log::error!("unable to save preference result to database: {}", e);
            panic!();
        }

        let mut rng = rand::thread_rng();

        self.engine
            .update_preference_outcome(&mut rng, self.choice.map_outcome(outcome));
    }

    fn rebuild_ranking(&mut self) -> anyhow::Result<OrderedItems<'_, T>> {
        let order = self.engine.rebuild_ranking()?;
        Ok(OrderedItems {
            items: self.items.as_slice(),
            order,
        })
    }

    fn order(&self) -> OrderedItems<'_, T> {
        let order = &self.engine.order;
        OrderedItems {
            items: self.items.as_slice(),
            order,
        }
    }
}

impl<T: CardDisplay> SiftApp<T> {
    pub fn update(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx<'_>) {
        let (lhs, rhs) = self.item_choices();
        let choice_selection = ChoiceSelection {
            lhs: ChoicePane {
                card: lhs.inner.make_card(),
            },
            rhs: ChoicePane {
                card: rhs.inner.make_card(),
            },
        };
        if let Some(outcome) = choice_selection.update(ui, ctx.hotkeys) {
            let outcome = match outcome {
                ChoiceOutcome::Left => Outcome::Left,
                ChoiceOutcome::Right => Outcome::Right,
                ChoiceOutcome::Equal => Outcome::Equal,
            };
            self.record_preference(outcome, ctx);
            if let Err(e) = self.rebuild_ranking() {
                log::error!("unable to rebuild ranking order: {}", e);
            }
            self.get_next().expect("ran out of things to rank");
        }
    }

    pub fn update_order(&mut self, ui: &mut egui::Ui) {
        let order = self.order();
        order.update(ui);
    }
}

impl<'a, T: CardDisplay> OrderedItems<'a, T> {
    pub fn update(&self, ui: &mut egui::Ui) {
        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style);
        let num_rows = self.items.len();
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show_rows(ui, row_height, num_rows, |ui, row_range| {
                for row in row_range {
                    let idx = self.order.translate(row);
                    let card = self.items[idx].inner.make_card();
                    card.update_short(ui);
                }
            });
    }
}
