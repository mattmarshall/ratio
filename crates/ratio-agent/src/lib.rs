//! The chat demo: a model configuring a fund's books, on the record.
//!
//! PLAN.md's five-minute script has a person talking to Claude, which drafts a
//! posting rule, has it checked, and cannot approve it. `demo/rehearse.sh`
//! proves that happens; this makes it something you can watch.
//!
//! # The tools are not a copy
//!
//! Every tool call goes through `ratio_mcp::dispatch`, and the tool list comes
//! from `ratio_mcp::tools()`. That matters more than it looks: the fence works
//! by **absence** — `approve_rule` is not in the list — so a second,
//! hand-maintained copy of the list for the chat path would be a second place
//! for it to reappear. There is one list, and this reads it.
//!
//! What a viewer sees is therefore evidence about the MCP server, not a
//! dramatization of it. The same is true of the model: it is told nothing
//! about approval it could not discover by calling the tool and being refused.
//!
//! # Why the model runs on Bedrock
//!
//! The demo is public, so it cannot ask a visitor for an API key, and it must
//! not carry one that a visitor could extract. Bedrock is reached with the
//! Lambda execution role's own credentials — there is no key in the image, in
//! an environment variable, or on the wire.
//!
//! # What bounds the cost
//!
//! A public endpoint that calls a paid model is the one part of this demo that
//! can spend real money, so the limits are explicit rather than incidental:
//!
//! * a small, fast model by default (`MODEL`);
//! * `MAX_TOKENS` per reply;
//! * `MAX_TURNS` tool round-trips before the loop stops on its own;
//! * `MAX_HISTORY` messages accepted from the caller, so a client cannot
//!   replay a conversation back at us to grow the context indefinitely;
//! * and, outside this crate, throttling on the API stage.
//!
//! None of these is the last line of defense — the account's budget is — but
//! each one turns "expensive" into "bounded".

use anyhow::{bail, Context, Result};
use aws_sdk_bedrockruntime::primitives::Blob;
use serde_json::{json, Value};

/// Haiku 4.5 through the US inference profile. Fast enough that a demo does
/// not stall on a cold model, and cheap enough that a public endpoint is not
/// a liability. `RATIO_MODEL` overrides it for a deeper demo.
pub const MODEL: &str = "us.anthropic.claude-haiku-4-5-20251001-v1:0";

/// Per-reply ceiling. Rules and explanations are short; anything approaching
/// this is a model that has lost the thread, and truncating it is correct.
const MAX_TOKENS: i32 = 1400;

/// Tool round-trips in one exchange. The scripted demo uses three or four
/// (list the chart, propose, check). Ten is generous; a model still going at
/// ten is looping.
const MAX_TURNS: usize = 10;

/// Messages accepted from the caller. The transcript lives in the browser, so
/// a client could otherwise send back an arbitrarily long history and bill it
/// to us.
const MAX_HISTORY: usize = 40;

/// The most a single user message may carry.
const MAX_MESSAGE_CHARS: usize = 4000;

const SYSTEM: &str = "\
You are helping a fund accountant set up the books for an investment fund, using \
Ratio — a double-entry accounting kernel whose conservation law is proved in Lean 4 \
and emitted to the Rust that runs.

Work the way a careful colleague would:

* Read the chart of accounts before drafting anything that posts to it.
* A posting rule is a WEIGHT VECTOR, not amounts: positive weights debit, negative \
  weights credit, and they must net to zero. That is what makes the entry balance at \
  any amount.
* Propose rules with `propose_rule`. It checks them and records a proposal. It does \
  NOT make them active.
* When a check comes back with a question, say so plainly and ask. Do not answer it \
  on the customer's behalf.

You cannot approve anything, and should not imply otherwise. A proposal becomes \
policy when a person runs `ratio approve <id>` at a terminal, having read it. If asked \
to approve, say that you are not able to and that it is deliberate: the point of this \
system is that a model can draft the arithmetic without being able to make the books \
wrong.

Be brief. A fund accountant is reading this, not an engineer — prefer 'the fee accrues \
daily on net assets' to 'the accrual rule computes rate_bp * days / 365'.";

/// One thing that happened, in order, for the transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The model said something.
    Said(String),
    /// The model called a tool, and what came back.
    Used {
        tool: String,
        input: String,
        output: String,
        /// True when the tool refused. The fence surfaces here.
        refused: bool,
    },
}

/// The result of one exchange.
#[derive(Debug, Clone)]
pub struct Reply {
    pub steps: Vec<Step>,
    /// The conversation so far, to be sent back on the next turn. Opaque to
    /// the caller; it is the Bedrock message list.
    pub history: Value,
    /// True when the loop stopped at `MAX_TURNS` rather than because the model
    /// was finished. Said out loud rather than hidden — a truncated exchange
    /// that looks complete is worse than one that admits it.
    pub truncated: bool,
}

/// Send one user message and run the tool loop to completion.
///
/// `history` is what a previous call returned, or `Value::Null` to start.
pub fn chat(book: &std::path::Path, history: &Value, message: &str) -> Result<Reply> {
    if message.trim().is_empty() {
        bail!("say something");
    }
    if message.len() > MAX_MESSAGE_CHARS {
        bail!("that message is {} characters; the limit is {MAX_MESSAGE_CHARS}", message.len());
    }

    let mut messages: Vec<Value> = match history {
        Value::Array(a) => a.clone(),
        _ => Vec::new(),
    };
    if messages.len() > MAX_HISTORY {
        bail!(
            "this conversation is {} messages; the limit is {MAX_HISTORY}. Reload to start again.",
            messages.len()
        );
    }
    messages.push(json!({"role": "user", "content": [{"type": "text", "text": message}]}));

    // The tool list, translated from MCP's shape to Bedrock's. Only the key
    // names differ — `inputSchema` becomes `input_schema` — and the list
    // itself is never rewritten here.
    let tools: Vec<Value> = ratio_mcp::tools()["tools"]
        .as_array()
        .context("tool list is not an array")?
        .iter()
        .map(|t| {
            json!({
                "name": t["name"],
                "description": t["description"],
                "input_schema": t["inputSchema"],
            })
        })
        .collect();

    let model = std::env::var("RATIO_MODEL").unwrap_or_else(|_| MODEL.to_string());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("starting the async runtime")?;
    let client = runtime.block_on(async {
        let cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        aws_sdk_bedrockruntime::Client::new(&cfg)
    });

    let mut steps = Vec::new();
    let mut truncated = true;

    for _ in 0..MAX_TURNS {
        let body = json!({
            "anthropic_version": "bedrock-2023-05-31",
            "max_tokens": MAX_TOKENS,
            "system": SYSTEM,
            "tools": tools,
            "messages": messages,
        });

        let response = runtime.block_on(async {
            client
                .invoke_model()
                .model_id(&model)
                .content_type("application/json")
                .body(Blob::new(serde_json::to_vec(&body)?))
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("{}", describe_sdk_error(&e)))
        })?;

        let out: Value = serde_json::from_slice(response.body().as_ref())
            .context("Bedrock returned something that is not JSON")?;
        let content = out["content"].as_array().cloned().unwrap_or_default();

        // Record what the model said before running any tool, so the
        // transcript reads in the order it happened.
        for block in &content {
            if block["type"] == "text" {
                if let Some(t) = block["text"].as_str() {
                    if !t.trim().is_empty() {
                        steps.push(Step::Said(t.trim().to_string()));
                    }
                }
            }
        }
        messages.push(json!({"role": "assistant", "content": content}));

        let calls: Vec<&Value> = content.iter().filter(|b| b["type"] == "tool_use").collect();
        if calls.is_empty() {
            truncated = false;
            break;
        }

        let mut results = Vec::new();
        for call in calls {
            let name = call["name"].as_str().unwrap_or("");
            let input = call["input"].clone();

            // The one dispatcher. A refusal is returned to the model as a tool
            // RESULT with is_error, not as a transport failure — so it reads
            // the sentence, and the transcript shows it being refused rather
            // than the conversation ending.
            let (text, refused) = match ratio_mcp::dispatch(book, name, &input) {
                Ok(t) => (t, false),
                Err(e) => (format!("{e:#}"), true),
            };

            steps.push(Step::Used {
                tool: name.to_string(),
                input: compact(&input),
                output: text.clone(),
                refused,
            });
            results.push(json!({
                "type": "tool_result",
                "tool_use_id": call["id"],
                "content": [{"type": "text", "text": text}],
                "is_error": refused,
            }));
        }
        messages.push(json!({"role": "user", "content": results}));
    }

    Ok(Reply {
        steps,
        history: Value::Array(messages),
        truncated,
    })
}

/// A one-line rendering of a tool's arguments, for the transcript.
fn compact(input: &Value) -> String {
    match input {
        Value::Object(o) if o.is_empty() => String::new(),
        Value::Object(o) => o
            .iter()
            .map(|(k, v)| match v.as_str() {
                // A rule's TOML is the interesting part and is multi-line;
                // show it whole rather than as an escaped blob.
                Some(s) if s.contains('\n') => format!("{k}:\n{s}"),
                Some(s) => format!("{k}: {s}"),
                None => format!("{k}: {v}"),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

/// Turn an SDK error into something a demo audience can act on.
///
/// The default `Display` for a Bedrock error is the enum variant, which tells
/// a viewer nothing. The two that actually happen — the model not being
/// enabled in the account, and throttling — have specific fixes.
fn describe_sdk_error<E: std::fmt::Debug>(e: &E) -> String {
    let raw = format!("{e:?}");
    // The one a NEW ACCOUNT hits, and the least self-explanatory. Bedrock
    // requires a one-time Anthropic use-case form per account, submitted in
    // the console; until it is, every invoke fails as ResourceNotFound — which
    // reads as "wrong model id" and sends you looking in the wrong place.
    //
    // ⚠ There is a grace window. This account answered a tool-use call
    // correctly, and refused the identical call forty minutes later. A working
    // Bedrock call is therefore NOT evidence that the form has been submitted.
    if raw.contains("use case details") || raw.contains("ResourceNotFoundException") {
        "This account has not submitted Bedrock's one-time Anthropic use-case form, so \
         the model cannot be called yet. It is a console action: Bedrock → Model access. \
         Everything else on this demo works without it — the tools, the books and the \
         fence are all local."
            .to_string()
    } else if raw.contains("AccessDeniedException") {
        "Bedrock refused the call. The execution role needs bedrock:InvokeModel, and the \
         model must be enabled in this account's Bedrock model access."
            .to_string()
    } else if raw.contains("ThrottlingException") {
        "Bedrock is throttling. Wait a moment and send it again.".to_string()
    } else if raw.contains("ValidationException") {
        format!("Bedrock rejected the request: {raw}")
    } else {
        format!("Bedrock call failed: {raw}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These do not call Bedrock. What is worth testing here is the wiring that
    // can be wrong without anyone noticing — the tool list the model is handed,
    // and the limits — not that AWS returns what AWS returns.

    #[test]
    fn the_model_is_handed_the_mcp_tool_list_and_nothing_else() {
        let tools: Vec<String> = ratio_mcp::tools()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(tools.len(), 8, "{tools:?}");
        assert!(tools.contains(&"propose_rule".to_string()));
        // Drafting a MAPPING is the same shape as drafting a rule, and sits
        // behind the same fence: it writes a proposal a person approves.
        assert!(tools.contains(&"propose_template".to_string()));
        // The fence, at the surface the model actually sees. If `approve_rule`
        // were ever added to the MCP list, this demo would hand it to a model
        // — so the assertion belongs here as well as in ratio-mcp.
        assert!(
            !tools.iter().any(|t| t.contains("approve")),
            "an approval tool reached the model: {tools:?}"
        );
    }

    #[test]
    fn every_mcp_tool_translates_to_a_bedrock_tool() {
        // Bedrock rejects a tool without a schema, and it does so on the FIRST
        // call of a demo rather than at build time.
        for t in ratio_mcp::tools()["tools"].as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            assert!(t["description"].is_string(), "{name} has no description");
            assert_eq!(t["inputSchema"]["type"], "object", "{name} schema");
        }
    }

    #[test]
    fn an_empty_or_oversized_message_is_refused_before_any_spend() {
        let book = std::env::temp_dir().join("ratio-agent-guard");
        assert!(chat(&book, &Value::Null, "   ").is_err());
        let long = "x".repeat(MAX_MESSAGE_CHARS + 1);
        let e = chat(&book, &Value::Null, &long).unwrap_err().to_string();
        assert!(e.contains("limit"), "{e}");
    }

    #[test]
    fn an_overlong_history_is_refused_before_any_spend() {
        // Without this a client can replay its own transcript back at us,
        // growing the context — and the bill — without limit.
        let book = std::env::temp_dir().join("ratio-agent-guard");
        let history = Value::Array(vec![
            json!({"role": "user", "content": []});
            MAX_HISTORY + 1
        ]);
        let e = chat(&book, &history, "hello").unwrap_err().to_string();
        assert!(e.contains("limit"), "{e}");
    }

    #[test]
    fn tool_arguments_render_readably() {
        assert_eq!(compact(&json!({})), "");
        assert_eq!(compact(&json!({"account": 10})), "account: 10");
        assert_eq!(compact(&json!({"id": "fee"})), "id: fee");
        // A rule's TOML is the interesting part of the transcript.
        let s = compact(&json!({"toml": "[[rule]]\nid = \"x\""}));
        assert!(s.starts_with("toml:\n[[rule]]"), "{s}");
    }

    #[test]
    fn sdk_errors_say_what_to_do() {
        #[derive(Debug)]
        struct E(&'static str);
        assert!(describe_sdk_error(&E("AccessDeniedException")).contains("model access"));
        assert!(describe_sdk_error(&E("ThrottlingException")).contains("Wait"));
    }
}
