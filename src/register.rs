#[cfg(target_os = "macos")]
use crate::config::Paths;
#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::{Result, bail};
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::process::Command;

const LABEL: &str = "dev.sisyphus.daemon";
const SERVICE_ENV: &str = "SISYPHUS_REGISTERED_SERVICE";

pub fn register_autostart() -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        bail!("sisyphus register is currently implemented for macOS LaunchAgent only");
    }

    #[cfg(target_os = "macos")]
    {
        let paths = Paths::resolve()?;
        paths.ensure_base_dir()?;

        let exe = std::env::current_exe().context("failed to resolve current executable")?;
        let launch_agents_dir = dirs::home_dir()
            .context("failed to resolve home directory")?
            .join("Library")
            .join("LaunchAgents");
        fs::create_dir_all(&launch_agents_dir)
            .with_context(|| format!("failed to create {}", launch_agents_dir.display()))?;

        let plist_path = launch_agents_dir.join(format!("{LABEL}.plist"));
        fs::write(
            &plist_path,
            render_launch_agent_plist(
                exe.to_string_lossy().as_ref(),
                paths.stdout_log_path.to_string_lossy().as_ref(),
                paths.stderr_log_path.to_string_lossy().as_ref(),
            ),
        )
        .with_context(|| format!("failed to write {}", plist_path.display()))?;

        let uid = unsafe { libc::getuid() };
        let domain = format!("gui/{uid}");

        let _ = Command::new("launchctl")
            .args(["bootout", &domain, plist_path.to_string_lossy().as_ref()])
            .status();

        let status = Command::new("launchctl")
            .args(["bootstrap", &domain, plist_path.to_string_lossy().as_ref()])
            .status()
            .context("failed to execute launchctl bootstrap")?;

        if !status.success() {
            bail!("launchctl bootstrap failed for {}", plist_path.display());
        }

        println!("Registered Sisyphus autostart at {}", plist_path.display());
        Ok(())
    }
}

pub fn render_launch_agent_plist(exe: &str, stdout_log: &str, stderr_log: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>serve</string>
    <string>--daemon</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>{SERVICE_ENV}</key>
    <string>1</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{stdout_log}</string>
  <key>StandardErrorPath</key>
  <string>{stderr_log}</string>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_agent_runs_serve_daemon_with_service_env() {
        let plist = render_launch_agent_plist("/bin/sisyphus", "/tmp/out.log", "/tmp/err.log");
        assert!(plist.contains("<string>/bin/sisyphus</string>"));
        assert!(plist.contains("<string>serve</string>"));
        assert!(plist.contains("<string>--daemon</string>"));
        assert!(plist.contains(SERVICE_ENV));
    }
}
