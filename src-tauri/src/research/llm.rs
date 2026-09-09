//! The research LLM call. Consumes only a `SafePortfolioContext` plus fetched
//! headlines — never balances, share counts, or account identifiers — and
//! returns a structured analysis.
//!
//! Two backends: the Anthropic Messages API (bring-your-own-key) or a local
//! Ollama server. Both are asked for strict JSON.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::http::HttpClient;
use crate::research::news::NewsItem;
use crate::research::safe_context::SafePortfolioContext;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HoldingNote {
    pub ticker: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Idea {
    pub ticker: String,
    #[serde(default)]
    pub name: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchAnalysis {
    /// Markdown synthesis of what's going on across the portfolio.
    pub summary: String,
    /// Concentration / correlation risk callout.
    #[serde(default)]
    pub concentration_risk: String,
    #[serde(default)]
    pub per_holding: Vec<HoldingNote>,
    #[serde(default)]
    pub ideas: Vec<Idea>,
}

#[derive(Debug, Clone)]
pub enum LlmProvider {
    Anthropic { api_key: String, model: String },
    Ollama { url: String, model: String },
}

impl LlmProvider {
    pub fn model(&self) -> &str {
        match self {
            Self::Anthropic { model, .. } | Self::Ollama { model, .. } => model,
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::Anthropic { .. } => "anthropic",
            Self::Ollama { .. } => "ollama",
        }
    }
}

const SYSTEM: &str = "You are an equity-research assistant embedded in a personal finance app. \
You are given ONLY ticker symbols and whole-number allocation percentages for the user's portfolio, \
plus a list of recent news headlines. You never receive dollar amounts, share counts, or account names, \
and you must not ask for them or guess at them. \
Some holdings may include a rounded unrealized gain/loss percentage vs. cost basis (\"up 40% vs cost\"); \
when present you may weigh it for things like trimming winners or tax-loss harvesting, but you still \
never see dollar amounts and must not estimate them. \
Give a grounded, skeptical synthesis — surface real catalysts and risks from the headlines, note \
concentration or correlation risk from the allocations, and suggest a few companies worth researching \
next with a one-line rationale each. This is not personalized financial advice and you should say so briefly.";

/// Appended to the prompt for providers without native structured output.
const JSON_SHAPE: &str = "Respond with ONLY a JSON object, no prose or code fences outside it, matching exactly:\n\
{\"summary\": string (markdown), \"concentration_risk\": string, \
\"per_holding\": [{\"ticker\": string, \"note\": string}], \
\"ideas\": [{\"ticker\": string, \"name\": string, \"rationale\": string}]}";

/// JSON-schema form of `ResearchAnalysis` for Anthropic tool use.
fn analysis_tool() -> Value {
    json!({
        "name": "submit_analysis",
        "description": "Return the portfolio research analysis.",
        "input_schema": {
            "type": "object",
            "properties": {
                "summary": { "type": "string", "description": "Markdown synthesis across the portfolio." },
                "concentration_risk": { "type": "string" },
                "per_holding": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "ticker": { "type": "string" }, "note": { "type": "string" } },
                        "required": ["ticker", "note"]
                    }
                },
                "ideas": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "ticker": { "type": "string" },
                            "name": { "type": "string" },
                            "rationale": { "type": "string" }
                        },
                        "required": ["ticker", "rationale"]
                    }
                }
            },
            "required": ["summary", "per_holding", "ideas"]
        }
    })
}

fn build_user_prompt(ctx: &SafePortfolioContext, news: &[NewsItem]) -> String {
    let holdings = ctx
        .holdings
        .iter()
        .map(|h| {
            let sector = h.sector.as_deref().map(|s| format!(" ({s})")).unwrap_or_default();
            let gain = match h.gain_pct {
                Some(g) if g >= 0 => format!(", up {g}% vs cost"),
                Some(g) => format!(", down {}% vs cost", g.abs()),
                None => String::new(),
            };
            format!("- {} {}%{sector}{gain}", h.ticker, h.allocation_pct)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let watchlist = if ctx.watchlist.is_empty() {
        "(none)".to_string()
    } else {
        ctx.watchlist.join(", ")
    };

    let headlines = if news.is_empty() {
        "(no headlines were retrieved)".to_string()
    } else {
        news.iter()
            .take(80)
            .map(|n| {
                format!(
                    "- [{}] {} — {} ({})",
                    n.ticker, n.headline, n.source, n.published_at
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Portfolio allocations:\n{holdings}\n\nWatchlist: {watchlist}\n\nRecent headlines:\n{headlines}"
    )
}

/// Scan from the first `{` and return the substring up to its matching `}`,
/// ignoring braces inside strings. `None` if it's never closed (truncated).
fn balanced_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, b) in s.bytes().enumerate().skip(start) {
        if in_str {
            match b {
                _ if escape => escape = false,
                b'\\' => escape = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Pull the JSON object out of a model response that may be fenced, prefixed
/// with prose, or followed by a trailing explanation.
fn extract_json(text: &str) -> AppResult<Value> {
    let t = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Ok(v);
    }
    if let Some(obj) = balanced_object(t) {
        if let Ok(v) = serde_json::from_str::<Value>(&obj) {
            return Ok(v);
        }
    }
    let snippet: String = t.chars().take(300).collect();
    let hint = if t.contains('{') && balanced_object(t).is_none() {
        " (the response looks cut off — try again)"
    } else {
        ""
    };
    Err(AppError::Other(format!(
        "the research model did not return valid JSON{hint}. It replied: {snippet}"
    )))
}

fn parse_analysis(text: &str) -> AppResult<ResearchAnalysis> {
    let v = extract_json(text)?;
    serde_json::from_value(v).map_err(|e| AppError::Other(format!("unexpected analysis shape: {e}")))
}

/// LLM generation can take much longer than the shared client's default; give
/// the research call its own budget.
const LLM_TIMEOUT: Duration = Duration::from_secs(150);

pub async fn run_analysis(
    http: &HttpClient,
    provider: &LlmProvider,
    ctx: &SafePortfolioContext,
    news: &[NewsItem],
) -> AppResult<ResearchAnalysis> {
    complete(http, provider, build_user_prompt(ctx, news)).await
}

/// A follow-up turn: the model gets the same portfolio context plus its own
/// previous analysis, and revises it to answer the user's question.
pub async fn run_followup(
    http: &HttpClient,
    provider: &LlmProvider,
    ctx: &SafePortfolioContext,
    news: &[NewsItem],
    prior: &ResearchAnalysis,
    question: &str,
) -> AppResult<ResearchAnalysis> {
    let prior_json = serde_json::to_string_pretty(prior).unwrap_or_default();
    let base = build_user_prompt(ctx, news);
    let user = format!(
        "{base}\n\nYour previous analysis (JSON):\n{prior_json}\n\n\
         The user's follow-up question:\n{question}\n\n\
         Return a revised full analysis in the same structure that answers the question. \
         Keep the parts that still hold, and fold your answer into `summary` so it reads as a \
         single coherent write-up (lead with the answer)."
    );
    complete(http, provider, user).await
}

async fn complete(
    http: &HttpClient,
    provider: &LlmProvider,
    user: String,
) -> AppResult<ResearchAnalysis> {
    match provider {
        LlmProvider::Anthropic { api_key, model } => {
            let body = json!({
                "model": model,
                "max_tokens": 4096,
                "system": SYSTEM,
                "messages": [{ "role": "user", "content": user }],
                "tools": [analysis_tool()],
                "tool_choice": { "type": "tool", "name": "submit_analysis" },
            });
            let resp = http
                .post("https://api.anthropic.com/v1/messages")?
                .header("x-api-key", api_key.trim())
                .header("anthropic-version", "2023-06-01")
                .timeout(LLM_TIMEOUT)
                .json(&body)
                .send()
                .await?;
            let status = resp.status();
            let v: Value = resp.json().await?;
            if !status.is_success() {
                let msg = v
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("request failed");
                return Err(AppError::Provider {
                    provider: "anthropic".into(),
                    code: v
                        .get("error")
                        .and_then(|e| e.get("type"))
                        .and_then(|t| t.as_str())
                        .map(String::from),
                    message: msg.to_string(),
                });
            }
            let stop_reason = v.get("stop_reason").and_then(|s| s.as_str()).unwrap_or("");
            // The reply is a tool_use block whose `input` is the analysis object.
            let input = v
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|a| {
                    a.iter().find_map(|b| {
                        (b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                            .then(|| b.get("input").cloned())
                            .flatten()
                    })
                });
            match input {
                Some(obj) => serde_json::from_value(obj)
                    .map_err(|e| AppError::Other(format!("unexpected analysis shape: {e}"))),
                None if stop_reason == "max_tokens" => Err(AppError::Other(
                    "the analysis was truncated by the model's length limit — try again".into(),
                )),
                None => {
                    // fall back to any text the model produced
                    let text = v
                        .get("content")
                        .and_then(|c| c.as_array())
                        .and_then(|a| {
                            a.iter().find_map(|b| b.get("text").and_then(|t| t.as_str()))
                        })
                        .unwrap_or_default();
                    parse_analysis(text)
                }
            }
        }
        LlmProvider::Ollama { url, model } => {
            let endpoint = format!("{}/api/chat", url.trim_end_matches('/'));
            let body = json!({
                "model": model,
                "stream": false,
                "format": "json",
                "options": { "temperature": 0.0, "num_predict": 3072 },
                "messages": [
                    { "role": "system", "content": SYSTEM },
                    { "role": "user", "content": format!("{user}\n\n{JSON_SHAPE}") },
                ],
            });
            let resp = http.post(&endpoint)?.timeout(LLM_TIMEOUT).json(&body).send().await?;
            let status = resp.status();
            let v: Value = resp.json().await?;
            if !status.is_success() {
                return Err(AppError::Provider {
                    provider: "ollama".into(),
                    code: None,
                    message: v
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("request failed")
                        .to_string(),
                });
            }
            let text = v
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or_default();
            if text.trim().is_empty() {
                return Err(AppError::Other(
                    "the local model returned an empty response".into(),
                ));
            }
            parse_analysis(text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let raw = "```json\n{\"summary\":\"ok\",\"concentration_risk\":\"low\",\"per_holding\":[],\"ideas\":[{\"ticker\":\"nvda\",\"name\":\"Nvidia\",\"rationale\":\"AI\"}]}\n```";
        let a = parse_analysis(raw).unwrap();
        assert_eq!(a.summary, "ok");
        assert_eq!(a.ideas[0].ticker, "nvda");
    }

    #[test]
    fn parses_json_with_surrounding_prose() {
        let raw = "Sure! Here you go:\n{\"summary\":\"s\",\"per_holding\":[],\"ideas\":[]}\nHope that helps.";
        let a = parse_analysis(raw).unwrap();
        assert_eq!(a.summary, "s");
        assert!(a.concentration_risk.is_empty());
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_analysis("I cannot help with that.").is_err());
    }

    #[test]
    fn balanced_object_stops_at_matching_brace() {
        let s = "{\"a\":{\"b\":\"}\"},\"c\":1} trailing junk } more";
        assert_eq!(balanced_object(s).unwrap(), "{\"a\":{\"b\":\"}\"},\"c\":1}");
    }

    #[test]
    fn truncated_json_gives_a_cutoff_hint() {
        let err = parse_analysis("{\"summary\":\"the market is").unwrap_err().to_string();
        assert!(err.contains("cut off"), "{err}");
    }

    #[test]
    fn parses_prefilled_anthropic_style_reply() {
        // what run_analysis builds after gluing "{" back on
        let glued = format!("{{{}", "\"summary\":\"s\",\"per_holding\":[],\"ideas\":[]}");
        assert_eq!(parse_analysis(&glued).unwrap().summary, "s");
    }
}
