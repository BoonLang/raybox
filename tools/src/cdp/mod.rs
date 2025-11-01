use anyhow::{Context, Result};
use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::js_protocol::runtime::{
    EventConsoleApiCalled, EventExceptionThrown, RemoteObject,
};
use futures::StreamExt;
use std::time::Duration;
use tokio::time::timeout;

/// Console message from the browser
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    pub level: String,
    pub text: String,
    #[allow(dead_code)]
    pub timestamp: std::time::SystemTime,
}

/// Exception thrown in the browser
#[derive(Debug, Clone)]
pub struct BrowserException {
    pub message: String,
    pub stack_trace: Option<String>,
}

/// Monitor browser console and performance via Chrome DevTools Protocol
pub struct ConsoleMonitor {
    browser: Browser,
}

impl ConsoleMonitor {
    /// Connect to Chrome's debugging port, or launch a new instance
    pub async fn connect(port: u16) -> Result<Self> {
        // First try to connect to existing Chrome with CDP enabled
        let url = format!("http://localhost:{}", port);

        let (browser, mut handler) = match Browser::connect(&url).await {
            Ok(result) => result,
            Err(_) => {
                // If connection fails, launch our own Chrome instance with WebGPU flags
                use chromiumoxide::browser::BrowserConfig;

                // CRITICAL: WebGPU requires these Chrome flags (see CLAUDE.md)
                let webgpu_flags = vec![
                    "--enable-unsafe-webgpu",
                    "--enable-webgpu-developer-features",
                    "--enable-features=Vulkan,VulkanFromANGLE",
                    "--enable-vulkan",
                    "--use-angle=vulkan",
                    "--disable-software-rasterizer",
                    "--ozone-platform=x11",  // Linux only, but harmless on other platforms
                ];

                Browser::launch(
                    BrowserConfig::builder()
                        .with_head() // Show browser window
                        .args(webgpu_flags)
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to build browser config: {}", e))?
                )
                .await
                .context("Failed to launch Chrome")?
            }
        };

        // Spawn handler task to process Chrome events
        // Note: chromiumoxide may log deserialization errors when Chrome 141+
        // sends CDP messages not yet in the library's protocol definitions.
        // These errors are harmless and don't affect functionality.
        tokio::spawn(async move {
            while handler.next().await.is_some() {
                // Handler events are processed by chromiumoxide internally
            }
        });

        Ok(Self { browser })
    }

    /// Check a page for console errors
    pub async fn check_page(&self, url: &str, wait_secs: u64) -> Result<PageReport> {
        let page = self
            .browser
            .new_page(url)
            .await
            .context("Failed to create new page")?;

        // Enable Runtime domain to receive console events
        page.enable_runtime().await?;
        page.enable_log().await?;

        // Listen for console messages
        let mut console_rx = page.event_listener::<EventConsoleApiCalled>().await?;

        // Listen for exceptions
        let mut exception_rx = page.event_listener::<EventExceptionThrown>().await?;

        let mut messages = Vec::new();
        let mut exceptions = Vec::new();

        // Wait for page to initialize and collect messages
        let wait_duration = Duration::from_secs(wait_secs);
        let timeout_result = timeout(wait_duration, async {
            loop {
                tokio::select! {
                    Some(event) = console_rx.next() => {
                        let level = format!("{:?}", event.r#type);

                        // Extract all arguments and join them
                        let text = if event.args.is_empty() {
                            "<empty>".to_string()
                        } else {
                            event.args
                                .iter()
                                .filter_map(|arg| extract_text(arg))
                                .collect::<Vec<_>>()
                                .join(" ")
                        };

                        messages.push(ConsoleMessage {
                            level,
                            text,
                            timestamp: std::time::SystemTime::now(),
                        });
                    }
                    Some(event) = exception_rx.next() => {
                        let message = event.exception_details.text.clone();
                        let stack_trace = event.exception_details.stack_trace
                            .as_ref()
                            .map(|st| format!("{:?}", st));

                        exceptions.push(BrowserException {
                            message,
                            stack_trace,
                        });
                    }
                    else => break,
                }
            }
        })
        .await;

        if timeout_result.is_err() {
            // Timeout reached, that's ok - we collected what we could
        }

        Ok(PageReport {
            url: url.to_string(),
            messages,
            exceptions,
        })
    }

    /// Take a screenshot of the page
    pub async fn screenshot(&self, url: &str) -> Result<Vec<u8>> {
        let page = self.browser.new_page(url).await?;

        // Wait for page load
        page.wait_for_navigation().await?;

        // Additional wait for rendering
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Capture screenshot
        let screenshot = page
            .screenshot(
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams::default(),
            )
            .await?;

        Ok(screenshot)
    }

    /// Get performance metrics from the page
    pub async fn get_performance_metrics(&self, url: &str) -> Result<PerformanceMetrics> {
        use chromiumoxide::cdp::browser_protocol::performance::{EnableParams, GetMetricsParams};

        let page = self.browser.new_page(url).await?;

        // Enable Performance domain
        page.execute(EnableParams::default()).await?;

        // Wait for page to load and render
        page.wait_for_navigation().await?;
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Get metrics
        let metrics = page.execute(GetMetricsParams::default()).await?;

        // Parse metrics
        let mut cpu_time = 0.0;
        let mut used_heap = 0.0;
        let mut total_heap = 0.0;

        for metric in metrics.result.metrics {
            match metric.name.as_str() {
                "TaskDuration" => cpu_time += metric.value,
                "JSHeapUsedSize" => used_heap = metric.value,
                "JSHeapTotalSize" => total_heap = metric.value,
                _ => {}
            }
        }

        Ok(PerformanceMetrics {
            cpu_time_ms: cpu_time * 1000.0,
            used_heap_mb: used_heap / 1024.0 / 1024.0,
            total_heap_mb: total_heap / 1024.0 / 1024.0,
        })
    }

    /// Profile CPU usage while interacting with the page
    pub async fn profile_cpu(&self, url: &str, duration_secs: u64) -> Result<CpuProfile> {
        use chromiumoxide::cdp::js_protocol::profiler::{
            EnableParams, StartParams, StopParams, SetSamplingIntervalParams,
        };

        let page = self.browser.new_page(url).await?;

        // Enable Profiler domain
        page.execute(EnableParams::default()).await?;

        // Set sampling interval (1000 microseconds = 1ms)
        page.execute(SetSamplingIntervalParams::new(1000)).await?;

        // Start CPU profiling
        page.execute(StartParams::default()).await?;

        // Wait for page load and profile for specified duration
        page.wait_for_navigation().await?;
        tokio::time::sleep(Duration::from_secs(duration_secs)).await;

        // Stop profiling and get results
        let result = page.execute(StopParams::default()).await?;

        Ok(CpuProfile {
            profile: serde_json::to_string_pretty(&result.profile)
                .unwrap_or_else(|_| format!("{:?}", result.profile)),
        })
    }
}

/// Report of a page check
#[derive(Debug)]
pub struct PageReport {
    pub url: String,
    pub messages: Vec<ConsoleMessage>,
    pub exceptions: Vec<BrowserException>,
}

impl PageReport {
    /// Get only error messages
    pub fn errors(&self) -> Vec<&ConsoleMessage> {
        self.messages
            .iter()
            .filter(|m| m.level.contains("error") || m.level.contains("Error"))
            .collect()
    }

    /// Check if there are any errors or exceptions
    pub fn has_errors(&self) -> bool {
        !self.errors().is_empty() || !self.exceptions.is_empty()
    }

    /// Print a summary report
    pub fn print_summary(&self) {
        println!("\n📊 Browser Console Report");
        println!("   URL: {}", self.url);
        println!("   Messages: {} total", self.messages.len());
        println!("   Errors: {}", self.errors().len());
        println!("   Exceptions: {}", self.exceptions.len());

        if !self.errors().is_empty() {
            println!("\n❌ Console Errors:");
            for msg in self.errors() {
                println!("   [{}] {}", msg.level, msg.text);
            }
        }

        if !self.exceptions.is_empty() {
            println!("\n💥 Exceptions:");
            for exc in &self.exceptions {
                println!("   {}", exc.message);
                if let Some(stack) = &exc.stack_trace {
                    println!("   Stack: {}", stack);
                }
            }
        }

        if !self.has_errors() {
            println!("   ✅ No errors detected!");
        }
    }
}

/// Performance metrics from the page
#[derive(Debug)]
pub struct PerformanceMetrics {
    pub cpu_time_ms: f64,
    pub used_heap_mb: f64,
    pub total_heap_mb: f64,
}

impl PerformanceMetrics {
    pub fn print_summary(&self) {
        println!("\n⚡ Performance Metrics");
        println!("   CPU Time: {:.2} ms", self.cpu_time_ms);
        println!(
            "   Heap Usage: {:.2} MB / {:.2} MB ({:.1}%)",
            self.used_heap_mb,
            self.total_heap_mb,
            (self.used_heap_mb / self.total_heap_mb) * 100.0
        );
    }
}

/// CPU profile data
#[derive(Debug)]
pub struct CpuProfile {
    pub profile: String,
}

// Helper to extract text from RemoteObject
fn extract_text(obj: &RemoteObject) -> Option<String> {
    // Try to extract value as different types
    if let Some(value) = &obj.value {
        // String values
        if let Some(s) = value.as_str() {
            return Some(s.to_string());
        }
        // Number values
        if let Some(n) = value.as_f64() {
            return Some(n.to_string());
        }
        // Boolean values
        if let Some(b) = value.as_bool() {
            return Some(b.to_string());
        }
        // Null
        if value.is_null() {
            return Some("null".to_string());
        }
        // Objects/Arrays - try to serialize
        if let Ok(serialized) = serde_json::to_string(value) {
            return Some(serialized);
        }
    }

    // Fall back to description (for Error objects, DOM elements, etc.)
    obj.description.clone()
}
