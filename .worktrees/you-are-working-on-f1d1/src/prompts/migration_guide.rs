//! Migration guide prompt

use std::collections::HashMap;

use tower_mcp::{GetPromptResult, Prompt, PromptBuilder, PromptMessage, PromptRole};

pub fn build() -> Prompt {
    PromptBuilder::new("migration_guide")
        .description("Generate a migration guide for switching between two crates")
        .required_arg("from_crate", "The crate being replaced")
        .required_arg("to_crate", "The crate being adopted")
        .handler(|args: HashMap<String, String>| async move {
            let from_crate = args
                .get("from_crate")
                .map(|s| s.as_str())
                .unwrap_or("unknown");
            let to_crate = args
                .get("to_crate")
                .map(|s| s.as_str())
                .unwrap_or("unknown");

            let prompt = format!(
                "Please generate a migration guide for switching from '{}' to '{}'.\n\n\
                 Use the available tools to gather data:\n\
                 - compare_crates on both crates to get a side-by-side overview\n\
                 - get_crate_docs for '{}' to understand its API surface\n\
                 - get_crate_docs for '{}' to understand its API surface\n\n\
                 Then analyze and document the following:\n\n\
                 1. **Dependencies**: Compare dependency counts and notable differences\n\
                 2. **Features**: Feature flags available in each crate and equivalents\n\
                 3. **MSRV**: Minimum Supported Rust Version for each crate\n\
                 4. **License**: License compatibility concerns\n\
                 5. **Key Differences**: API changes, renamed types/functions, removed or added concepts\n\
                 6. **Migration Concerns**: Breaking changes, behavioral differences, known pitfalls\n\n\
                 Produce a structured migration guide with step-by-step instructions for \
                 switching from '{}' to '{}'.",
                from_crate,
                to_crate,
                from_crate,
                to_crate,
                from_crate,
                to_crate,
            );

            Ok(GetPromptResult {
                description: Some(format!(
                    "Migration guide from '{}' to '{}'",
                    from_crate, to_crate
                )),
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
