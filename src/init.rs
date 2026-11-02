//! The `init` subcommand: register this server with an MCP client.
//!
//! This exists for one reason a README snippet cannot cover. In stdio mode the
//! client needs the absolute path of this binary, and that path differs between
//! `cargo install`, Homebrew, and a source build. The running process is the
//! only thing that reliably knows where it lives, so it registers itself.
//!
//! Registration goes through each client's own CLI (`claude mcp add`,
//! `codex mcp add`) rather than by editing its config file. The client owns its
//! config schema and scope resolution, and a populated config is large enough
//! that rewriting it from here risks damaging unrelated settings.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Args as ClapArgs, ValueEnum};

/// Matches the error type used by the binary's `main`.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The hosted deployment, used by `--mode hosted`.
const HOSTED_URL: &str = "https://cratesio-mcp.fly.dev/";

/// Name the server is registered under unless `--name` overrides it.
const DEFAULT_NAME: &str = "cratesio-mcp";

/// Which client to configure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ClientArg {
    /// Claude Code, via `claude mcp add`.
    Claude,
    /// Codex, via `codex mcp add`.
    Codex,
    /// Every supported client whose CLI is on `PATH`.
    All,
}

/// How the client should reach the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// Launch this binary locally and speak over stdio.
    Stdio,
    /// Point at a streamable HTTP deployment.
    Hosted,
}

/// Claude Code configuration scope. Codex has no equivalent and ignores this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scope {
    Local,
    User,
    Project,
}

impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Scope::Local => "local",
            Scope::User => "user",
            Scope::Project => "project",
        }
    }
}

/// A client that `init` knows how to drive. `ClientArg::All` expands to these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Claude,
    Codex,
}

impl Target {
    /// Executable to look for on `PATH` and to spawn.
    fn program(self) -> &'static str {
        match self {
            Target::Claude => "claude",
            Target::Codex => "codex",
        }
    }

    /// Human-facing name, for progress and error messages.
    fn label(self) -> &'static str {
        match self {
            Target::Claude => "Claude Code",
            Target::Codex => "Codex",
        }
    }
}

impl ClientArg {
    /// Expand the `--client` selection into concrete clients.
    fn targets(self) -> Vec<Target> {
        match self {
            ClientArg::Claude => vec![Target::Claude],
            ClientArg::Codex => vec![Target::Codex],
            ClientArg::All => vec![Target::Claude, Target::Codex],
        }
    }

    /// Whether the user named a single client rather than asking for whatever
    /// is installed. A missing CLI is an error in the first case and a skip in
    /// the second.
    fn is_explicit(self) -> bool {
        !matches!(self, ClientArg::All)
    }
}

#[derive(ClapArgs, Debug)]
pub struct InitArgs {
    /// Client to configure
    #[arg(long, value_enum, default_value = "all")]
    pub client: ClientArg,

    /// How the client should reach the server
    #[arg(long, value_enum, default_value = "stdio")]
    pub mode: Mode,

    /// Name to register the server under
    #[arg(long, default_value = DEFAULT_NAME)]
    pub name: String,

    /// Server URL for `--mode hosted`
    #[arg(long, default_value = HOSTED_URL)]
    pub url: String,

    /// Claude Code configuration scope (ignored for Codex)
    #[arg(long, value_enum, default_value = "user")]
    pub scope: Scope,

    /// Print the exact command that would run, without running it
    #[arg(long)]
    pub dry_run: bool,

    /// Print config file snippets instead of invoking any client CLI.
    /// This is the only way to configure Claude Desktop, which has no CLI.
    #[arg(long)]
    pub print: bool,
}

/// A command to run: program plus arguments, already split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
}

impl Invocation {
    /// Render for display, quoting any argument containing whitespace so the
    /// printed line can be pasted into a shell.
    fn display(&self) -> String {
        let mut out = String::from(&self.program);
        for arg in &self.args {
            out.push(' ');
            if arg.is_empty() || arg.chars().any(char::is_whitespace) {
                out.push('"');
                out.push_str(arg);
                out.push('"');
            } else {
                out.push_str(arg);
            }
        }
        out
    }
}

/// Build the client CLI invocation that registers this server.
///
/// Pure: every input is explicit, including the binary path, so the result can
/// be asserted against without consulting `PATH` or spawning a process.
///
/// The target CLIs take:
///
/// ```text
/// claude mcp add [-s <scope>] [--transport http] <name> <commandOrUrl> [args...]
/// codex  mcp add <name> (--url <URL> | -- <COMMAND>...)
/// ```
pub fn invocation(
    target: Target,
    mode: Mode,
    name: &str,
    url: &str,
    scope: Scope,
    exe: &Path,
) -> Invocation {
    let exe = exe.display().to_string();

    // `--transport stdio` is already this binary's default. Registering it
    // explicitly keeps the client config honest if that default ever changes.
    let args = match (target, mode) {
        (Target::Claude, Mode::Stdio) => vec![
            "mcp".to_string(),
            "add".to_string(),
            "-s".to_string(),
            scope.as_str().to_string(),
            name.to_string(),
            "--".to_string(),
            exe,
            "--transport".to_string(),
            "stdio".to_string(),
        ],
        (Target::Claude, Mode::Hosted) => vec![
            "mcp".to_string(),
            "add".to_string(),
            "-s".to_string(),
            scope.as_str().to_string(),
            "--transport".to_string(),
            "http".to_string(),
            name.to_string(),
            url.to_string(),
        ],
        (Target::Codex, Mode::Stdio) => vec![
            "mcp".to_string(),
            "add".to_string(),
            name.to_string(),
            "--".to_string(),
            exe,
            "--transport".to_string(),
            "stdio".to_string(),
        ],
        (Target::Codex, Mode::Hosted) => vec![
            "mcp".to_string(),
            "add".to_string(),
            name.to_string(),
            "--url".to_string(),
            url.to_string(),
        ],
    };

    Invocation {
        program: target.program().to_string(),
        args,
    }
}

/// Config file snippet for a client, for `--print`.
///
/// Claude Code and Claude Desktop share the `mcpServers` JSON shape; Codex uses
/// a TOML table. String values go through `serde_json::to_string`, which quotes
/// and escapes them correctly for JSON and, for the characters that occur in
/// paths and URLs, for a TOML basic string as well.
pub fn snippet(target: Target, mode: Mode, name: &str, url: &str, exe: &Path) -> String {
    let exe = exe.display().to_string();

    match target {
        // Claude Code and Claude Desktop share this shape. Built through
        // serde_json rather than formatted by hand so the result is valid and
        // correctly escaped whatever the path contains.
        Target::Claude => {
            let entry = match mode {
                Mode::Stdio => serde_json::json!({
                    "type": "stdio",
                    "command": exe,
                    "args": ["--transport", "stdio"],
                }),
                Mode::Hosted => serde_json::json!({
                    "type": "http",
                    "url": url,
                }),
            };
            let doc = serde_json::json!({ "mcpServers": { name: entry } });
            serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
        }
        // A TOML table. Values go through serde_json::to_string, which quotes
        // and escapes correctly for a TOML basic string across the characters
        // that occur in paths and URLs.
        Target::Codex => {
            let quote = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s));
            match mode {
                Mode::Stdio => format!(
                    "[mcp_servers.{}]\ncommand = {}\nargs = [\"--transport\", \"stdio\"]",
                    name,
                    quote(&exe)
                ),
                Mode::Hosted => {
                    format!("[mcp_servers.{}]\nurl = {}", name, quote(url))
                }
            }
        }
    }
}

/// Whether `program` resolves to a file on `PATH`.
///
/// This is a deliberate stand-in for a `which` dependency: it answers the only
/// question `init` asks, which is whether to attempt the spawn at all. A false
/// positive degrades to the spawn failing with the CLI's own error.
fn on_path(program: &str) -> bool {
    match std::env::var_os("PATH") {
        Some(paths) => program_in(program, &paths),
        None => false,
    }
}

/// Whether `program` is a file in any directory of `paths`.
///
/// Split out from [`on_path`] so the search can be tested against a known set
/// of directories instead of mutating the process environment, which would
/// race with other tests in the same binary.
fn program_in(program: &str, paths: &OsStr) -> bool {
    std::env::split_paths(paths).any(|dir| dir.join(program).is_file())
}

/// Where each client keeps the config a `--print` snippet belongs in.
fn config_hint(target: Target) -> &'static str {
    match target {
        Target::Claude => {
            "~/.claude.json (user scope), or .mcp.json in the project root \
             (project scope). Claude Desktop uses the same shape in \
             claude_desktop_config.json."
        }
        Target::Codex => "~/.codex/config.toml",
    }
}

/// Run the `init` subcommand.
pub fn run(args: &InitArgs) -> Result<(), BoxError> {
    let exe = resolve_exe(args.mode)?;
    let targets = args.client.targets();

    if args.print {
        print_snippets(&targets, args, &exe);
        return Ok(());
    }

    let mut configured = Vec::new();
    let mut skipped = Vec::new();

    for target in &targets {
        let target = *target;

        if !on_path(target.program()) {
            if args.client.is_explicit() {
                return Err(format!(
                    "`{}` was not found on PATH, so {} cannot be configured \
                     automatically.\nRe-run with --print to get a config \
                     snippet to paste instead.",
                    target.program(),
                    target.label()
                )
                .into());
            }
            skipped.push(target);
            continue;
        }

        let inv = invocation(target, args.mode, &args.name, &args.url, args.scope, &exe);

        if args.dry_run {
            println!("{}", inv.display());
            continue;
        }

        let status = Command::new(&inv.program).args(&inv.args).status();
        match status {
            Ok(status) if status.success() => configured.push(target),
            Ok(status) => {
                return Err(format!(
                    "`{}` exited with {}. The command was:\n  {}",
                    target.program(),
                    status,
                    inv.display()
                )
                .into());
            }
            Err(e) => {
                return Err(format!(
                    "failed to run `{}`: {e}. The command was:\n  {}",
                    target.program(),
                    inv.display()
                )
                .into());
            }
        }
    }

    if args.dry_run {
        return Ok(());
    }

    report(&configured, &skipped, args);
    Ok(())
}

/// Absolute path of the running binary, needed only by stdio mode.
///
/// Hosted mode never spawns a local process, so a failure to resolve the path
/// there is not worth aborting over.
fn resolve_exe(mode: Mode) -> Result<PathBuf, BoxError> {
    match std::env::current_exe() {
        Ok(path) => Ok(path),
        Err(e) if matches!(mode, Mode::Hosted) => {
            let _ = e;
            Ok(PathBuf::from("cratesio-mcp"))
        }
        Err(e) => Err(format!(
            "could not determine the path of this binary, which stdio mode \
             needs in order to register it: {e}"
        )
        .into()),
    }
}

/// Emit a config snippet per requested client.
fn print_snippets(targets: &[Target], args: &InitArgs, exe: &Path) {
    for (i, target) in targets.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("# {} -- {}", target.label(), config_hint(*target));
        println!(
            "{}",
            snippet(*target, args.mode, &args.name, &args.url, exe)
        );
    }
}

/// Summarize what was configured and what was passed over.
fn report(configured: &[Target], skipped: &[Target], args: &InitArgs) {
    for target in configured {
        let how = match args.mode {
            Mode::Stdio => "stdio".to_string(),
            Mode::Hosted => format!("hosted at {}", args.url),
        };
        println!(
            "Registered `{}` with {} ({}).",
            args.name,
            target.label(),
            how
        );
    }

    for target in skipped {
        println!(
            "Skipped {}: `{}` is not on PATH.",
            target.label(),
            target.program()
        );
    }

    if configured.is_empty() {
        println!(
            "\nNo client CLI was found, so nothing was configured. \
             Re-run with --print to get config snippets to paste."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from("/usr/local/bin/cratesio-mcp")
    }

    #[test]
    fn claude_stdio_registers_the_binary_path() {
        let inv = invocation(
            Target::Claude,
            Mode::Stdio,
            "cratesio",
            HOSTED_URL,
            Scope::User,
            &exe(),
        );
        assert_eq!(inv.program, "claude");
        assert_eq!(
            inv.args,
            vec![
                "mcp",
                "add",
                "-s",
                "user",
                "cratesio",
                "--",
                "/usr/local/bin/cratesio-mcp",
                "--transport",
                "stdio",
            ]
        );
    }

    #[test]
    fn claude_hosted_uses_the_http_transport_flag() {
        let inv = invocation(
            Target::Claude,
            Mode::Hosted,
            "cratesio",
            "https://example.test/",
            Scope::User,
            &exe(),
        );
        assert_eq!(
            inv.args,
            vec![
                "mcp",
                "add",
                "-s",
                "user",
                "--transport",
                "http",
                "cratesio",
                "https://example.test/",
            ]
        );
        // The local binary path is irrelevant to a hosted registration.
        assert!(!inv.args.iter().any(|a| a.contains("cratesio-mcp")));
    }

    #[test]
    fn codex_stdio_passes_the_command_after_a_double_dash() {
        let inv = invocation(
            Target::Codex,
            Mode::Stdio,
            "cratesio",
            HOSTED_URL,
            Scope::User,
            &exe(),
        );
        assert_eq!(inv.program, "codex");
        assert_eq!(
            inv.args,
            vec![
                "mcp",
                "add",
                "cratesio",
                "--",
                "/usr/local/bin/cratesio-mcp",
                "--transport",
                "stdio",
            ]
        );
    }

    #[test]
    fn codex_hosted_uses_url_flag() {
        let inv = invocation(
            Target::Codex,
            Mode::Hosted,
            "cratesio",
            "https://example.test/",
            Scope::User,
            &exe(),
        );
        assert_eq!(
            inv.args,
            vec!["mcp", "add", "cratesio", "--url", "https://example.test/"]
        );
    }

    #[test]
    fn claude_scope_is_threaded_through() {
        for (scope, expected) in [
            (Scope::Local, "local"),
            (Scope::User, "user"),
            (Scope::Project, "project"),
        ] {
            let inv = invocation(
                Target::Claude,
                Mode::Stdio,
                "cratesio",
                HOSTED_URL,
                scope,
                &exe(),
            );
            let pos = inv.args.iter().position(|a| a == "-s").expect("-s present");
            assert_eq!(inv.args[pos + 1], expected);
        }
    }

    #[test]
    fn codex_ignores_scope_because_it_has_no_equivalent() {
        let inv = invocation(
            Target::Codex,
            Mode::Stdio,
            "cratesio",
            HOSTED_URL,
            Scope::Project,
            &exe(),
        );
        assert!(!inv.args.iter().any(|a| a == "-s"));
        assert!(!inv.args.iter().any(|a| a == "project"));
    }

    #[test]
    fn custom_name_is_used_verbatim() {
        let inv = invocation(
            Target::Claude,
            Mode::Stdio,
            "my-crates",
            HOSTED_URL,
            Scope::User,
            &exe(),
        );
        assert!(inv.args.contains(&"my-crates".to_string()));
        assert!(!inv.args.contains(&"cratesio".to_string()));
    }

    #[test]
    fn all_expands_to_both_clients_in_order() {
        assert_eq!(
            ClientArg::All.targets(),
            vec![Target::Claude, Target::Codex]
        );
        assert_eq!(ClientArg::Claude.targets(), vec![Target::Claude]);
        assert_eq!(ClientArg::Codex.targets(), vec![Target::Codex]);
    }

    #[test]
    fn only_all_is_treated_as_non_explicit() {
        assert!(!ClientArg::All.is_explicit());
        assert!(ClientArg::Claude.is_explicit());
        assert!(ClientArg::Codex.is_explicit());
    }

    #[test]
    fn display_quotes_paths_containing_spaces() {
        let inv = invocation(
            Target::Codex,
            Mode::Stdio,
            "cratesio",
            HOSTED_URL,
            Scope::User,
            Path::new("/Applications/My Tools/cratesio-mcp"),
        );
        let shown = inv.display();
        assert!(
            shown.contains("\"/Applications/My Tools/cratesio-mcp\""),
            "path with a space must be quoted, got: {shown}"
        );
    }

    #[test]
    fn claude_snippet_is_valid_json_naming_the_binary() {
        let out = snippet(Target::Claude, Mode::Stdio, "cratesio", HOSTED_URL, &exe());
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("snippet must be valid JSON");
        assert_eq!(
            parsed["mcpServers"]["cratesio"]["command"],
            "/usr/local/bin/cratesio-mcp"
        );
        assert_eq!(parsed["mcpServers"]["cratesio"]["args"][0], "--transport");
        // Matches the shape `claude mcp add` writes for a stdio server.
        assert_eq!(parsed["mcpServers"]["cratesio"]["type"], "stdio");
    }

    #[test]
    fn claude_hosted_snippet_is_valid_json_naming_the_url() {
        let out = snippet(
            Target::Claude,
            Mode::Hosted,
            "cratesio",
            "https://example.test/",
            &exe(),
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("snippet must be valid JSON");
        assert_eq!(parsed["mcpServers"]["cratesio"]["type"], "http");
        assert_eq!(
            parsed["mcpServers"]["cratesio"]["url"],
            "https://example.test/"
        );
    }

    #[test]
    fn snippet_escapes_a_path_containing_a_quote() {
        let out = snippet(
            Target::Claude,
            Mode::Stdio,
            "cratesio",
            HOSTED_URL,
            Path::new("/opt/we\"ird/cratesio-mcp"),
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("a quote in the path must stay valid JSON");
        assert_eq!(
            parsed["mcpServers"]["cratesio"]["command"],
            "/opt/we\"ird/cratesio-mcp"
        );
    }

    #[test]
    fn codex_snippets_declare_the_expected_toml_table() {
        let stdio = snippet(Target::Codex, Mode::Stdio, "cratesio", HOSTED_URL, &exe());
        assert!(stdio.starts_with("[mcp_servers.cratesio]"));
        assert!(stdio.contains("command = \"/usr/local/bin/cratesio-mcp\""));

        let hosted = snippet(
            Target::Codex,
            Mode::Hosted,
            "cratesio",
            "https://example.test/",
            &exe(),
        );
        assert!(hosted.starts_with("[mcp_servers.cratesio]"));
        assert!(hosted.contains("url = \"https://example.test/\""));
        assert!(!hosted.contains("command"));
    }

    #[test]
    fn hosted_mode_tolerates_an_unresolvable_binary_path() {
        // Only stdio registration needs the path, so hosted must not fail
        // even in an environment where current_exe is unavailable.
        assert!(resolve_exe(Mode::Hosted).is_ok());
    }

    #[test]
    fn program_in_finds_a_file_in_one_of_the_directories() {
        let dir = std::env::temp_dir().join("cratesio-mcp-init-program-in-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let bin = dir.join("definitely-a-fake-cli");
        std::fs::write(&bin, b"#!/bin/sh\n").expect("write stub");

        let paths = std::env::join_paths([Path::new("/nonexistent-dir"), dir.as_path()])
            .expect("join paths");

        assert!(
            program_in("definitely-a-fake-cli", &paths),
            "a file in a listed directory must be found"
        );
        assert!(
            !program_in("no-such-cli-anywhere", &paths),
            "an absent program must not be found"
        );

        let _ = std::fs::remove_file(&bin);
    }

    #[test]
    fn program_in_handles_an_empty_path_list() {
        let empty = std::ffi::OsString::new();
        assert!(!program_in("claude", &empty));
    }
}
