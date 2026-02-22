//! Compare crates prompt

use std::collections::HashMap;

use tower_mcp::{GetPromptResult, Prompt, PromptBuilder, PromptMessage, PromptRole};

pub fn build() -> Prompt {
    PromptBuilder::new("compare_crates")
        .description("Compare two or more crates for a specific use case")
        .required_arg("crates", "Comma-separated list of crate names to compare")
        .required_arg("use_case", "What you want to use these crates for")
        .handler(|args: HashMap<String, String>| async move {
            let crates = args.get("crates").map(|s| s.as_str()).unwrap_or("");
            let use_case = args
                .get("use_case")
                .map(|s| s.as_str())
                .unwrap_or("general use");

            let crate_list: Vec<&str> = crates.split(',').map(|s| s.trim()).collect();

            let prompt = format!(
                "Please compare the following Rust crates for {}: {}\n\n\
                 For each crate, use the available tools to gather data and evaluate:\n\n\
                 1. **Features**: What each crate offers\n\
                 2. **API Design**: Ease of use, ergonomics\n\
                 3. **Performance**: Any known performance characteristics\n\
                 4. **Dependencies**: Use get_dependencies to check dependency counts\n\
                 5. **Maintenance**: Use get_crate_versions and get_owners to assess\n\
                 6. **Popularity**: Use get_downloads and get_reverse_dependencies\n\n\
                 Provide a comparison table and a final recommendation with reasoning.",
                use_case,
                crate_list.join(", ")
            );

            Ok(GetPromptResult {
                description: Some(format!("Compare crates: {}", crates)),
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
