//! Turning tokens into dollars, from rates the user supplies.
//!
//! Pacer has no price data and cannot get any offline: a transcript line
//! carries no `costUSD`, Claude Code's own `stats-cache.json` records 0 for
//! every model on a subscription plan, and its model catalog has no price
//! fields at all. Inventing a table would put a confident wrong number on
//! screen. So the `model_prices` map in `config.json` starts empty, the
//! diagnostics card shows tokens until someone fills it in, and an empty
//! table means "never set up" rather than "everything is free".
//!
//! This lives in the TUI and not in `pacer-core` because no price crosses
//! the wire and no daemon code reads one.

use pacer_core::TokenUsage;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Anthropic's published cache rates as multiples of the input rate, which
/// is what the `None` arms below fall back to.
const CACHE_WRITE_5M: f64 = 1.25;
const CACHE_WRITE_1H: f64 = 2.0;
const CACHE_READ: f64 = 0.1;

/// Dollars per million tokens. The cache rates default to the multiples
/// above, so a user types two numbers and overrides only what differs.
///
/// Reasoning effort is **not** a coefficient: thinking tokens are already
/// inside `output_tokens`, so effort shows up as more output on its own.
/// There is deliberately no `effort` field.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    /// `Option` rather than a serde default, because a default value
    /// cannot be told apart from a user who typed that same value.
    #[serde(default)]
    pub cache_write_5m: Option<f64>,
    #[serde(default)]
    pub cache_write_1h: Option<f64>,
    #[serde(default)]
    pub cache_read: Option<f64>,
}

/// The configured rates, keyed by however much of a model id the user chose
/// to name.
#[derive(Default)]
pub struct PriceTable(BTreeMap<String, ModelPrice>);

impl PriceTable {
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        Self(cfg.model_prices.clone())
    }

    /// True when the user configured any price at all. The card draws the
    /// cost column only then: an empty table means the feature was never
    /// set up, not that everything is free.
    pub fn configured(&self) -> bool {
        !self.0.is_empty()
    }

    /// The longest configured key the observed model id starts with.
    ///
    /// `claude-sonnet-4-5` prices `claude-sonnet-4-5-20250929`, and a user
    /// who wants one snapshot priced differently adds the full id, which
    /// wins by length. Exact match would silently unprice the whole card
    /// on every model refresh; `contains` would match two families with no
    /// deterministic tiebreak.
    fn rate(&self, model: &str) -> Option<&ModelPrice> {
        self.0
            .iter()
            .filter(|(key, _)| model.starts_with(key.as_str()))
            .max_by_key(|(key, _)| key.len())
            .map(|(_, price)| price)
    }

    /// `None` when **any** model in `usage` is unpriced: a partial total is
    /// a wrong number wearing a right one's clothes.
    ///
    /// An empty usage is `Some(0.0)` — it contains no unpriced model, so
    /// that is a measured zero and correct to show.
    pub fn cost_usd(&self, usage: &TokenUsage) -> Option<f64> {
        let mut total = 0.0;
        for tokens in &usage.by_model {
            let rate = self.rate(&tokens.model)?;
            let per =
                |count: u64, usd_per_million: f64| count as f64 * usd_per_million / 1_000_000.0;
            total += per(tokens.input, rate.input);
            total += per(tokens.output, rate.output);
            total += per(
                tokens.cache_write_5m,
                rate.cache_write_5m.unwrap_or(rate.input * CACHE_WRITE_5M),
            );
            total += per(
                tokens.cache_write_1h,
                rate.cache_write_1h.unwrap_or(rate.input * CACHE_WRITE_1H),
            );
            total += per(
                tokens.cache_read,
                rate.cache_read.unwrap_or(rate.input * CACHE_READ),
            );
        }
        Some(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pacer_core::ModelTokens;

    fn table(entries: &[(&str, ModelPrice)]) -> PriceTable {
        PriceTable(
            entries
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.clone()))
                .collect(),
        )
    }

    fn price(input: f64, output: f64) -> ModelPrice {
        ModelPrice {
            input,
            output,
            cache_write_5m: None,
            cache_write_1h: None,
            cache_read: None,
        }
    }

    fn spent(model: &str) -> TokenUsage {
        TokenUsage {
            by_model: vec![ModelTokens {
                model: model.into(),
                input: 1_000_000,
                output: 1_000_000,
                cache_read: 1_000_000,
                cache_write_5m: 1_000_000,
                cache_write_1h: 1_000_000,
            }],
        }
    }

    /// The cache tiers are the part that can be a 10x error: a read priced
    /// at the input rate overstates the bill tenfold.
    #[test]
    fn cache_tiers_default_to_multiples_of_input() {
        let t = table(&[("claude-sonnet-4-5", price(3.0, 15.0))]);
        // 3 input + 15 output + 0.3 read + 3.75 write-5m + 6 write-1h.
        let cost = t.cost_usd(&spent("claude-sonnet-4-5")).unwrap();
        assert!((cost - 28.05).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn an_explicit_rate_overrides_its_default() {
        let t = table(&[(
            "m",
            ModelPrice {
                cache_read: Some(1.5),
                ..price(3.0, 15.0)
            },
        )]);
        // Only the read line changes: 0.3 becomes 1.5.
        let cost = t.cost_usd(&spent("m")).unwrap();
        assert!((cost - 29.25).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn the_longest_matching_key_wins() {
        let t = table(&[
            ("claude-opus", price(1.0, 1.0)),
            ("claude-opus-5-20260101", price(100.0, 100.0)),
        ]);
        let usage = TokenUsage {
            by_model: vec![ModelTokens {
                model: "claude-opus-5-20260101".into(),
                input: 1_000_000,
                ..Default::default()
            }],
        };
        assert_eq!(t.cost_usd(&usage), Some(100.0));
    }

    /// `—` and `$0.00` are different facts and must stay apart: one
    /// unpriced model unprices the whole figure, and a conversation that
    /// spent nothing costs a measured zero.
    #[test]
    fn one_unpriced_model_unprices_the_total() {
        let t = table(&[("known", price(3.0, 15.0))]);
        let mixed = TokenUsage {
            by_model: vec![
                ModelTokens {
                    model: "known".into(),
                    input: 1_000_000,
                    ..Default::default()
                },
                ModelTokens {
                    model: "stranger".into(),
                    input: 1_000_000,
                    ..Default::default()
                },
            ],
        };
        assert_eq!(t.cost_usd(&mixed), None);
        assert_eq!(t.cost_usd(&TokenUsage::default()), Some(0.0));
    }

    #[test]
    fn an_empty_table_is_not_configured() {
        assert!(!table(&[]).configured());
        assert!(table(&[("m", price(1.0, 1.0))]).configured());
    }
}
