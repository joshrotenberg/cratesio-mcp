//! Evaluate dependencies prompt

use std::collections::HashMap;

use tower_mcp::{GetPromptResult, Prompt, PromptBuilder, PromptMessage, PromptRole};

pub fn build() -> Prompt {
    PromptBuilder::new("evaluate_dependencies")
        .description("Evaluate a project's dependencies for health, security, and maintenance")
        .required_arg("crates", "Comma-separated list of dependency names")
        .optional_arg("use_case", "Project context (e.g. 'production web service')")
        .handler(|args: HashMap<String, String>| async move {
            let crates = args.get("crates").map(|s| s.as_str()).unwrap_or("unknown");
            let use_case = args.get("use_case");

            let mut prompt = format!(
                "Please evaluate the following Rust dependencies: {}\n\n\
                 For each crate, run `crate_health_check` to gather comprehensive data, then assess:\n\n\
                 1. **Staleness**: Flag crates with no recent releases or inactive maintenance\n\
                 2. **Security**: Identify any known vulnerabilities or audit concerns\n\
                 3. **Adoption**: Flag low-download or low-ecosystem-usage crates\n\
                 4. **MSRV**: Note crates missing a declared minimum supported Rust version\n\
                 5. **Alternatives**: Suggest better-maintained or more popular alternatives for any problematic dependencies\n\n\
                 After evaluating each dependency individually, provide a summary of overall dependency health \
                 and prioritized recommendations for action.",
                crates
            );

            if let Some(uc) = use_case {
                prompt.push_str(&format!(
                    "\n\nProject context: {}\n\
                     Please tailor your evaluation and recommendations to this use case.",
                    uc
                ));
            }

            Ok(GetPromptResult {
                description: Some("Evaluate dependency health, security, and maintenance".to_string()),
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
