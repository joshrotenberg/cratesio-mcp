//! Stack review prompt

use std::collections::HashMap;

use tower_mcp::{GetPromptResult, Prompt, PromptBuilder, PromptMessage, PromptRole};

pub fn build() -> Prompt {
    PromptBuilder::new("stack_review")
        .description("Evaluate a set of crates as a cohesive stack for compatibility and health")
        .required_arg("crates", "Comma-separated list of crate names")
        .optional_arg(
            "use_case",
            "What the stack is for (e.g. 'async web service', 'CLI tool')",
        )
        .handler(|args: HashMap<String, String>| async move {
            let crates = args.get("crates").map(|s| s.as_str()).unwrap_or("");
            let use_case = args.get("use_case");

            let crate_list: Vec<&str> = crates.split(',').map(|s| s.trim()).collect();

            let mut prompt = format!(
                "Please evaluate the following Rust crates as a cohesive stack: {}\n\n\
                 Use the available tools to perform a thorough stack review:\n\n\
                 1. **Health Check**: Run crate_health_check on each crate to assess individual health\n\
                 2. **Dependency Analysis**: Run get_dependencies on each crate to identify shared \
                 transitive dependencies and potential version conflicts\n\
                 3. **Overlap Detection**: Check for overlapping functionality across the stack \
                 (e.g. two HTTP clients, two async runtimes, two logging frameworks) and flag redundancies\n\
                 4. **MSRV Compatibility**: Evaluate the collective minimum supported Rust version \
                 (MSRV) across all crates and flag any incompatibilities\n\
                 5. **Stack Summary**: Summarize overall stack health, highlight concerns, and \
                 provide recommendations for improving cohesion or replacing problematic crates",
                crate_list.join(", ")
            );

            if let Some(uc) = use_case {
                prompt.push_str(&format!(
                    "\n\nThis stack is intended for: {}\n\
                     Please evaluate specifically for this use case and note whether the chosen \
                     crates are well-suited for it.",
                    uc
                ));
            }

            Ok(GetPromptResult {
                description: Some(format!("Review crate stack: {}", crates)),
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
