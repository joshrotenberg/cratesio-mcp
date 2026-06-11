//! Recommend crates prompt

use std::collections::HashMap;

use tower_mcp::{GetPromptResult, Prompt, PromptBuilder, PromptMessage, PromptRole};

pub fn build() -> Prompt {
    PromptBuilder::new("recommend_crates")
        .description("Find and evaluate crates for a given use case")
        .required_arg(
            "use_case",
            "What you want to build (e.g. 'REST API with auth', 'CLI argument parsing')",
        )
        .optional_arg("max_results", "How many crates to evaluate (default 5)")
        .handler(|args: HashMap<String, String>| async move {
            let use_case = args
                .get("use_case")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            let max_results = args
                .get("max_results")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(5);

            let prompt = format!(
                "Please recommend the best Rust crates for the following use case: {}\n\n\
                 Follow these steps:\n\n\
                 1. **Identify keywords**: Extract relevant search terms from the use case \
                 (e.g. framework names, problem domain, key features needed)\n\n\
                 2. **Search for candidates**: Use search_crates with those keywords to find \
                 up to {} candidate crates\n\n\
                 3. **Health check top results**: Run crate_health_check on the most promising \
                 candidates to get comprehensive quality data\n\n\
                 4. **Compare on key dimensions**:\n\
                 - Downloads (total and recent trends)\n\
                 - Freshness (time since last release, release frequency)\n\
                 - Dependency weight (number of transitive deps)\n\
                 - MSRV (minimum supported Rust version)\n\
                 - Maintenance signals (open issues, last commit)\n\n\
                 5. **Provide a ranked recommendation**: List crates from most to least \
                 recommended, with a brief rationale for each including key tradeoffs \
                 (e.g. 'battle-tested but heavier deps vs. newer but more lightweight')\n\n\
                 Focus on actionable guidance: which crate should they reach for first, \
                 and when might they prefer an alternative?",
                use_case, max_results
            );

            Ok(GetPromptResult {
                description: Some(format!("Recommend crates for: {}", use_case)),
                messages: vec![PromptMessage {
                    role: PromptRole::User,
                    content: tower_mcp::protocol::Content::Text {
                        text: prompt,
                        annotations: None,
                        meta: None,
                    },
                    meta: None,
                }],
                meta: None,
            })
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn required_args_appear_in_text() {
        let prompt = build();
        let mut args = HashMap::new();
        args.insert("use_case".to_string(), "CLI argument parsing".to_string());
        let result = prompt.get(args).await.unwrap();
        let text = result.first_message_text().expect("expected message text");
        assert!(text.contains("CLI argument parsing"));
        assert!(text.contains('5'));
    }

    #[tokio::test]
    async fn optional_max_results_appears_in_text() {
        let prompt = build();
        let mut args = HashMap::new();
        args.insert("use_case".to_string(), "REST API".to_string());
        args.insert("max_results".to_string(), "10".to_string());
        let result = prompt.get(args).await.unwrap();
        let text = result.first_message_text().expect("expected message text");
        assert!(text.contains("REST API"));
        assert!(text.contains("10"));
    }

    #[tokio::test]
    async fn description_is_set() {
        let prompt = build();
        let mut args = HashMap::new();
        args.insert("use_case".to_string(), "async HTTP client".to_string());
        let result = prompt.get(args).await.unwrap();
        assert!(result.description.is_some());
        assert!(result.description.unwrap().contains("async HTTP client"));
    }
}
