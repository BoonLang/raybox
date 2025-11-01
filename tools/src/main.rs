mod cdp;
mod commands;
mod layout;
mod wasm_bindgen;
mod wasm_opt;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "canvas-tools")]
#[command(about = "TodoMVC Canvas Renderer Development Tools", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract DOM layout from HTML/CSS analysis
    ExtractDom {
        /// Output JSON file path
        #[arg(short, long)]
        output: String,
    },

    /// Compare two layout JSON files
    CompareLayouts {
        /// Reference layout JSON file
        #[arg(short, long)]
        reference: String,

        /// Actual layout JSON file to compare
        #[arg(short, long)]
        actual: String,

        /// Optional: Export visual diff JSON
        #[arg(short, long)]
        diff_output: Option<String>,
    },

    /// Generate HTML visualization of layout
    VisualizeLayout {
        /// Input layout JSON file
        #[arg(short, long)]
        input: String,

        /// Output HTML file
        #[arg(short, long)]
        output: String,
    },

    /// Start HTTP server for development
    Serve {
        /// Directory to serve
        #[arg(default_value = "dist")]
        directory: String,

        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    /// Capture screenshot via Chrome DevTools Protocol
    Screenshot {
        /// URL to capture
        #[arg(short, long)]
        url: String,

        /// Output PNG file
        #[arg(short, long)]
        output: String,

        /// Viewport width
        #[arg(long, default_value = "1920")]
        width: u32,

        /// Viewport height
        #[arg(long, default_value = "1080")]
        height: u32,
    },

    /// Watch files and auto-rebuild
    Watch {
        /// Directory to watch
        #[arg(default_value = ".")]
        directory: String,

        /// Command to run on changes
        #[arg(short, long)]
        command: String,
    },

    /// Compare two images pixel-by-pixel with SSIM
    PixelDiff {
        /// Reference image (PNG/JPG)
        #[arg(short, long)]
        reference: String,

        /// Current image to compare
        #[arg(short, long)]
        current: String,

        /// Optional: Output diff image showing differences
        #[arg(short, long)]
        output: Option<String>,

        /// Similarity threshold (0.0-1.0, default: 0.95)
        #[arg(short, long, default_value = "0.95")]
        threshold: f64,
    },

    /// Build WASM renderer
    WasmBuild {
        /// Build in release mode with optimizations
        #[arg(short, long)]
        release: bool,
    },

    /// Start WASM development server with auto-reload
    /// Check browser console for errors via Chrome DevTools Protocol
    CheckConsole {
        /// URL to check
        #[arg(short, long, default_value = "http://localhost:8000")]
        url: String,

        /// Chrome debugging port
        #[arg(long, default_value = "9222")]
        port: u16,

        /// How long to wait for page load (seconds)
        #[arg(short, long, default_value = "3")]
        wait: u64,

        /// Also capture screenshot
        #[arg(short, long)]
        screenshot: bool,

        /// Also get performance metrics
        #[arg(short = 'm', long)]
        performance: bool,

        /// Also run CPU profiling for N seconds
        #[arg(short = 'p', long)]
        profile: Option<u64>,
    },

    WasmStart {
        /// Build in release mode with optimizations
        #[arg(short, long)]
        release: bool,

        /// Open browser automatically
        #[arg(short, long)]
        open: bool,

        /// Port to listen on
        #[arg(short, long, default_value = "8000")]
        port: u16,
    },

    /// Run integration tests (cross-platform Rust implementation)
    IntegrationTest {
        /// URL to test
        #[arg(short, long, default_value = "http://localhost:8000")]
        url: String,
    },
}

fn main() -> Result<()> {
    // Configure logging - filter out harmless chromiumoxide deserialization warnings
    // These occur when Chrome 141+ sends CDP protocol messages not yet in chromiumoxide's
    // auto-generated bindings. They don't affect functionality.
    env_logger::Builder::from_default_env()
        .filter_module("chromiumoxide::conn", log::LevelFilter::Warn)
        .filter_module("chromiumoxide::handler", log::LevelFilter::Warn)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::ExtractDom { output } => {
            commands::extract_dom::run(&output)?;
        }

        Commands::CompareLayouts {
            reference,
            actual,
            diff_output,
        } => {
            commands::compare_layouts::run(&reference, &actual, diff_output.as_deref())?;
        }

        Commands::VisualizeLayout { input, output } => {
            commands::visualize_layout::run(&input, &output)?;
        }

        Commands::Serve { directory, port } => {
            commands::serve::run(&directory, port)?;
        }

        Commands::Screenshot {
            url,
            output,
            width,
            height,
        } => {
            commands::screenshot::run(&url, &output, width, height)?;
        }

        Commands::Watch { directory, command } => {
            commands::watch::run(&directory, &command)?;
        }

        Commands::PixelDiff {
            reference,
            current,
            output,
            threshold,
        } => {
            commands::pixel_diff::run(&reference, &current, output.as_deref(), threshold)?;
        }

        Commands::CheckConsole {
            url,
            port,
            wait,
            screenshot,
            performance,
            profile,
        } => {
            // Run async CDP checking
            tokio::runtime::Runtime::new()?.block_on(async {
                eprintln!("\n📝 Note: chromiumoxide may log deserialization errors below.");
                eprintln!("   This occurs when Chrome 141+ sends CDP messages not yet in the library.");
                eprintln!("   These errors are harmless and don't affect console monitoring.\n");

                let monitor = cdp::ConsoleMonitor::connect(port).await?;

                // Check console
                let report = monitor.check_page(&url, wait).await?;
                report.print_summary();

                // Screenshot if requested
                if screenshot {
                    println!("\n📸 Taking screenshot...");
                    match monitor.screenshot(&url).await {
                        Ok(data) => {
                            let path = "screenshot.png";
                            std::fs::write(path, data)?;
                            println!("   ✅ Screenshot saved: {}", path);
                        }
                        Err(e) => eprintln!("   ❌ Screenshot failed: {}", e),
                    }
                }

                // Performance metrics if requested
                if performance {
                    println!("\n⚡ Getting performance metrics...");
                    match monitor.get_performance_metrics(&url).await {
                        Ok(metrics) => metrics.print_summary(),
                        Err(e) => eprintln!("   ❌ Performance check failed: {}", e),
                    }
                }

                // CPU profiling if requested
                if let Some(duration) = profile {
                    println!("\n🔬 Running CPU profiling for {} seconds...", duration);
                    match monitor.profile_cpu(&url, duration).await {
                        Ok(cpu_profile) => {
                            let path = "cpu_profile.json";
                            std::fs::write(path, &cpu_profile.profile)?;
                            println!("   ✅ CPU profile saved: {}", path);
                        }
                        Err(e) => eprintln!("   ❌ CPU profiling failed: {}", e),
                    }
                }

                // Exit with error code if there were errors
                if report.has_errors() {
                    std::process::exit(1);
                }

                Ok::<(), anyhow::Error>(())
            })?;
        }

        Commands::WasmBuild { release } => {
            commands::wasm_build::run(release)?;
        }

        Commands::WasmStart { release, open, port } => {
            commands::wasm_start::run(release, open, port)?;
        }

        Commands::IntegrationTest { url } => {
            commands::integration_test::run(&url)?;
        }
    }

    Ok(())
}
