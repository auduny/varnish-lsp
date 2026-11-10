use zed_extension_api::{self as zed, LanguageServerId, Result};

struct VarnishExtension;

impl zed::Extension for VarnishExtension {
    fn new() -> Self {
        VarnishExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree.which("varnishls").ok_or_else(|| {
            "varnishls not found in PATH. Install it with: cargo install --git https://github.com/auduny/varnish-lsp".to_string()
        })?;

        Ok(zed::Command {
            command: path,
            args: vec!["lsp".to_string(), "--stdio".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(VarnishExtension);
