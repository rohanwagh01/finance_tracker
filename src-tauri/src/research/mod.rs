//! Research feature: market news + LLM synthesis, fed only by `safe_context`.
//!
//! `safe_context` is the sole path from portfolio data to any outbound service;
//! it reduces holdings to `{ ticker, allocation_pct, sector }`. `news` fetches
//! headlines for those tickers; `llm` asks Anthropic or Ollama for a structured
//! analysis of the two together.

pub mod llm;
pub mod news;
pub mod safe_context;
