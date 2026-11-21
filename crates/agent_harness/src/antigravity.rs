use std::path::PathBuf;

use crate::acp::AcpAgent;
use crate::{HarnessId, PermissionMode, ReasoningLevel};

pub static ANTIGRAVITY: AcpAgent = AcpAgent {
    id: HarnessId::Antigravity,
    name: "Antigravity",
    program: "agy",
    args: &["acp"],
    executable_env: "AGY_EXECUTABLE",
    install_paths,
    searched: "searched PATH, ~/.local/bin, /opt/homebrew/bin and /usr/local/bin; \
               set AGY_EXECUTABLE to override",
    update_args: &["update"],
    modes: &[
        (PermissionMode::Ask, "default"),
        (PermissionMode::AcceptEdits, "accept-edits"),
        (PermissionMode::Auto, "auto"),
        (PermissionMode::FullAccess, "auto"),
    ],
    steers_mid_turn: true,
    sign_in_hint: i18n::n!(
        "Antigravity isn't signed in. Run `agy` in a terminal to sign in, then try again."
    ),
    command_output,
    reasoning_levels: &[
        ReasoningLevel::Low,
        ReasoningLevel::Medium,
        ReasoningLevel::High,
    ],
};

fn install_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = crate::executable::home_dir() {
        paths.push(home.join(".local").join("bin").join("agy"));
    }
    paths.push(PathBuf::from("/opt/homebrew/bin/agy"));
    paths.push(PathBuf::from("/usr/local/bin/agy"));
    paths
}

/// Antigravity puts timestamps and the exit code ahead of a command's real output.
fn command_output(text: &str) -> &str {
    text.split_once("\nCommand output:\n")
        .map_or(text, |(_, output)| output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_output_drops_the_status_lines() {
        let text = "Created At: now\nCompleted At: now\nCommand exited with code 0.\n\nCommand output:\nhello.txt\r\n";
        assert_eq!(super::command_output(text), "hello.txt\r\n");
        assert_eq!(super::command_output("plain"), "plain");
    }
}
