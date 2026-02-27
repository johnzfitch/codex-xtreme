//! CODEX//XTREME - Neo Tokyo Y2K Edition
//!
//! A cyberpunk-themed TUI for building patched Codex binaries.

use codex_xtreme::core::check_prerequisites;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let dev_mode = args.iter().any(|a| a == "--dev" || a == "-d");

    let cargo_jobs = {
        let mut found: Option<usize> = None;
        for (idx, arg) in args.iter().enumerate() {
            let value: Option<&str> = if arg == "--jobs" || arg == "-j" {
                Some(
                    args.get(idx + 1)
                        .ok_or_else(|| anyhow::anyhow!("Missing value for {arg}"))?
                        .as_str(),
                )
            } else if let Some(rest) = arg.strip_prefix("--jobs=") {
                Some(rest)
            } else if let Some(rest) = arg.strip_prefix("-j") {
                if rest.is_empty() {
                    None
                } else {
                    Some(rest)
                }
            } else {
                None
            };
            let Some(value) = value else { continue };
            let jobs: usize = value
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid value for --jobs/-j: {value}"))?;
            if jobs == 0 {
                return Err(anyhow::anyhow!("Invalid value for --jobs/-j: must be >= 1"));
            }
            if found.replace(jobs).is_some() {
                return Err(anyhow::anyhow!(
                    "Multiple --jobs/-j values provided; use only one"
                ));
            }
        }
        found
    };

    if let Err(err) = check_prerequisites() {
        eprintln!("{err}");
        std::process::exit(1);
    }

    codex_xtreme::tui::run_app(dev_mode, cargo_jobs).await
}
