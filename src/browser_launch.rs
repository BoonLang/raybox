//! Shared Chromium/WebGPU launch helpers for Raybox web flows.

use crate::control::{BlockingWsClient, Command, Response, ResponseMessage};
use anyhow::{anyhow, bail, Context, Result};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use url::Url;

pub const DEFAULT_DEBUG_PORT: u16 = 9222;

const DEFAULT_CONTROL_READY_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct BrowserLaunchConfig {
    pub url: String,
    pub chrome_bin: Option<PathBuf>,
    pub debug_port: u16,
    pub headless: bool,
    pub app_mode: bool,
    pub compat: bool,
    pub use_default_profile: bool,
    pub user_data_dir: Option<PathBuf>,
    pub extra_args: Vec<String>,
}

impl Default for BrowserLaunchConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8000".to_string(),
            chrome_bin: None,
            debug_port: DEFAULT_DEBUG_PORT,
            headless: false,
            app_mode: false,
            compat: cfg!(target_os = "linux"),
            use_default_profile: false,
            user_data_dir: None,
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct BrowserLaunch {
    pub child: Child,
    pub chrome_bin: PathBuf,
    pub url: String,
    pub debug_port: u16,
    pub args: Vec<String>,
    pub owned_profile_dir: Option<PathBuf>,
}

pub fn build_launch_url(
    base_url: &str,
    demo: Option<u8>,
    control: bool,
    hotreload: bool,
) -> Result<String> {
    let mut url = Url::parse(base_url).with_context(|| format!("invalid URL: {base_url}"))?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(demo) = demo {
            pairs.append_pair("demo", &demo.to_string());
        }
        if control {
            pairs.append_pair("control", "1");
        }
        if hotreload {
            pairs.append_pair("hotreload", "1");
        }
    }
    Ok(url.into())
}

pub fn resolve_chrome_bin(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    if let Some(path) = env::var_os("RAYBOX_CHROME_BIN") {
        return Ok(PathBuf::from(path));
    }

    for candidate in ["google-chrome", "chromium", "chromium-browser"] {
        if let Some(path) = which_in_path(candidate) {
            return Ok(path);
        }
    }

    bail!("Could not find a Chromium binary. Set --chrome-bin or RAYBOX_CHROME_BIN.")
}

pub fn spawn_chromium(config: &BrowserLaunchConfig) -> Result<BrowserLaunch> {
    let chrome_bin = resolve_chrome_bin(config.chrome_bin.as_deref())?;
    let (args, owned_profile_dir) = build_chromium_args(config)?;

    let mut command = ProcessCommand::new(&chrome_bin);
    command.args(&args);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);

    let child = command
        .spawn()
        .with_context(|| format!("failed to launch Chromium at {}", chrome_bin.display()))?;

    Ok(BrowserLaunch {
        child,
        chrome_bin,
        url: config.url.clone(),
        debug_port: config.debug_port,
        args,
        owned_profile_dir,
    })
}

pub fn stop_browser(launch: &mut BrowserLaunch) {
    let _ = launch.child.kill();
    let _ = launch.child.wait();
    cleanup_profile_dir(launch.owned_profile_dir.as_deref());
}

pub fn cleanup_profile_dir(path: Option<&Path>) {
    if let Some(path) = path {
        let _ = fs::remove_dir_all(path);
    }
}

pub fn wait_for_control_ready(timeout: Duration) -> Result<ResponseMessage> {
    let started = Instant::now();
    let mut last_error: Option<anyhow::Error> = None;

    while started.elapsed() < timeout {
        match BlockingWsClient::new() {
            Ok(mut client) => match client.connect_local() {
                Ok(()) => match client
                    .send_command_with_timeout(Command::GetStatus, Duration::from_secs(2))
                {
                    Ok(response) => match response.response {
                        Response::Status { .. } => return Ok(response),
                        Response::Error { .. } => {
                            last_error = Some(anyhow!("control server reachable, but no web app is connected yet"))
                        }
                        _ => {
                            last_error = Some(anyhow!(
                                "control server returned an unexpected response while waiting for web status"
                            ))
                        }
                    },
                    Err(error) => last_error = Some(error),
                },
                Err(error) => last_error = Some(error),
            },
            Err(error) => last_error = Some(error),
        }

        thread::sleep(DEFAULT_POLL_INTERVAL);
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow!(
            "Timed out waiting for the web app to answer on the control server after {:?}",
            timeout
        )
    }))
}

pub fn default_control_ready_timeout() -> Duration {
    DEFAULT_CONTROL_READY_TIMEOUT
}

fn build_chromium_args(config: &BrowserLaunchConfig) -> Result<(Vec<String>, Option<PathBuf>)> {
    let owned_profile_dir = resolve_profile_dir(config)?;
    let mut args = vec![
        format!("--remote-debugging-port={}", config.debug_port),
        "--remote-debugging-address=127.0.0.1".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--enable-unsafe-webgpu".to_string(),
        "--enable-webgpu-developer-features".to_string(),
        "--disable-background-timer-throttling".to_string(),
        "--disable-renderer-backgrounding".to_string(),
        "--disable-backgrounding-occluded-windows".to_string(),
    ];

    if config.compat {
        args.extend([
            "--force-webgpu".to_string(),
            "--ignore-gpu-blocklist".to_string(),
            "--enable-vulkan".to_string(),
            "--use-angle=vulkan".to_string(),
            "--disable-software-rasterizer".to_string(),
            "--ozone-platform=x11".to_string(),
            "--enable-features=UnsafeWebGPU,SharedArrayBufferOnDesktop,Vulkan,VulkanFromANGLE,DefaultANGLEVulkan,UseSkiaRenderer".to_string(),
        ]);
    } else {
        args.push(
            "--enable-features=UnsafeWebGPU,SharedArrayBufferOnDesktop,UseSkiaRenderer".to_string(),
        );
    }

    if let Some(dir) = config.user_data_dir.as_ref().or(owned_profile_dir.as_ref()) {
        args.push(format!("--user-data-dir={}", dir.display()));
    }

    if config.headless {
        args.push("--headless=new".to_string());
    }

    if config.app_mode && !config.headless {
        args.push(format!("--app={}", config.url));
    } else {
        args.push(config.url.clone());
    }

    args.extend(config.extra_args.iter().cloned());

    Ok((args, owned_profile_dir))
}

fn resolve_profile_dir(config: &BrowserLaunchConfig) -> Result<Option<PathBuf>> {
    if config.user_data_dir.is_some() || config.use_default_profile {
        return Ok(None);
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dir = env::temp_dir().join(format!("raybox-chromium-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create browser profile dir {}", dir.display()))?;
    Ok(Some(dir))
}

fn which_in_path(binary: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{build_launch_url, BrowserLaunchConfig};

    #[test]
    fn build_launch_url_adds_expected_query_parameters() {
        let url = build_launch_url("http://127.0.0.1:8000", Some(8), true, true).unwrap();
        assert_eq!(url, "http://127.0.0.1:8000/?demo=8&control=1&hotreload=1");
    }

    #[test]
    fn browser_launch_defaults_enable_linux_compatibility() {
        let config = BrowserLaunchConfig::default();
        if cfg!(target_os = "linux") {
            assert!(config.compat);
        }
    }
}
