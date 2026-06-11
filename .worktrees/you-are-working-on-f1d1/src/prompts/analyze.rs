//! Analyze crate prompt

use std::collections::HashMap;

use tower_mcp::{GetPromptResult, Prompt, PromptBuilder, PromptMessage, PromptRole};

pub fn build() -> Prompt {
    PromptBuilder::new("analyze_crate")
        .description("Analyze a crate for quality, maintenance, and suitability")
        .required_arg("name", "Name of the crate to analyze")
        .optional_arg("use_case", "Your intended use case for the crate")
        .handler(|args: HashMap<String, String>| async move {
            let name = args.get("name").map(|s| s.as_str()).unwrap_or("unknown");
            let use_case = args.get("use_case");

            let mut prompt = format!(
                "Please analyze the Rust crate '{}' for the following aspects:\n\n\
                 1. **Quality**: Code quality, test coverage, documentation\n\
                 2. **Maintenance**: Activity level, responsiveness to issues\n\
                 3. **Popularity**: Download trends, community adoption\n\
                 4. **Alternatives**: Similar crates and how they compare\n\
                 5. **Recommendation**: Whether to use this crate\n\n\
                 Use the available tools to fetch data:\n\
                 - get_crate_info for details\n\
                 - get_crate_versions for release frequency\n\
                 - get_downloads for usage trends\n\
                 - get_reverse_dependencies for ecosystem impact\n\
                 - get_owners for maintainer info",
                name
            );

            if let Some(uc) = use_case {
                prompt.push_str(&format!(
                    "\n\nThe intended use case is: {}\n\
                     Please evaluate specifically for this use case.",
                    uc
                ));
            }

            Ok(GetPromptResult {
                description: Some(format!("Analyze the '{}' crate", name)),
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
