mod cdp;
mod commands;
mod layout;
mod layout_precise;
mod wasm_bindgen;
mod wasm_opt;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "raybox-tools")]
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

    /// Extract DOM layout via Chrome DevTools Protocol (browser-based, most accurate)
    ExtractDomCdp {
        /// URL to extract layout from
        #[arg(short, long)]
        url: String,

        /// Output JSON file path
        #[arg(short, long)]
        output: String,

        /// Viewport width
        #[arg(long, default_value = "700")]
        width: u32,

        /// Viewport height
        #[arg(long, default_value = "700")]
        height: u32,
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

        /// Connect to existing Chrome instance on this CDP port (e.g., 9333)
        /// If not specified, launches a new Chrome instance
        #[arg(long)]
        cdp_port: Option<u16>,
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

        /// Renderer to build and watch (classic or emergent)
        #[arg(long, default_value = "classic")]
        renderer: String,
    },

    /// Run integration tests (cross-platform Rust implementation)
    IntegrationTest {
        /// URL to test
        #[arg(short, long, default_value = "http://localhost:8000")]
        url: String,
    },

    /// Pixel diff images (exact match ratio)
    PixelDiffSimple {
        /// Reference image path
        #[arg(short, long)]
        reference: String,
        /// Candidate image path
        #[arg(short, long)]
        candidate: String,
        /// Minimum similarity (0.0-1.0). Default 0.97
        #[arg(long, default_value = "0.97")]
        threshold: f64,
    },

    /// Capture precise layout from reference HTML
    CaptureReference {
        /// HTML file to open (inside reference/)
        #[arg(short, long, default_value = "reference/html/todomvc_populated.html")]
        file: String,

        /// Optional layout JSON to use instead of DOM snapshot
        #[arg(long)]
        layout_json: Option<String>,

        /// Output JSON path
        #[arg(
            short,
            long,
            default_value = "reference/layouts/layout_precise_reference.json"
        )]
        out: String,

        /// Launch headed Chrome (default headless)
        #[arg(long, default_value_t = false)]
        headed: bool,

        /// Path to Chrome/Chromium binary
        #[arg(long)]
        chrome_path: Option<String>,
    },

    /// Capture precise layout from the renderer (requires running server)
    CaptureRenderer {
        /// URL of running renderer
        #[arg(short, long, default_value = "http://localhost:8000")]
        url: String,

        /// Output JSON path
        #[arg(
            short,
            long,
            default_value = "reference/layouts/layout_precise_renderer.json"
        )]
        out: String,

        /// Launch headed Chrome (default headless)
        #[arg(long, default_value_t = false)]
        headed: bool,

        /// Path to Chrome/Chromium binary
        #[arg(long)]
        chrome_path: Option<String>,
    },

    /// Diff two precise layout files
    DiffLayouts {
        /// Left JSON
        #[arg(short = 'a', long)]
        left: String,

        /// Right JSON
        #[arg(short = 'b', long)]
        right: String,

        /// Threshold in px
        #[arg(short, long, default_value = "0.1")]
        threshold: f64,
    },

    /// Generate MTSDF font atlas from TTF/OTF font file
    GenerateMtsdfAtlas {
        /// Path to font file (TTF/OTF)
        #[arg(short, long)]
        font: String,

        /// Output PNG file path
        #[arg(short, long)]
        output_png: String,

        /// Output JSON metadata file path
        #[arg(short = 'j', long)]
        output_json: String,

        /// Glyph size in pixels (default: 32)
        #[arg(long, default_value = "32")]
        glyph_size: u32,

        /// Padding around each glyph (default: 4)
        #[arg(long, default_value = "4")]
        padding: u32,

        /// SDF distance range (default: 4.0)
        #[arg(long, default_value = "4.0")]
        sdf_range: f32,

        /// Characters to include (default: ASCII printable 32-126)
        #[arg(long)]
        charset: Option<String>,
    },

    /// Generate SDF font atlas using fontdue (simpler than MTSDF)
    GenerateSdfAtlas {
        /// Path to font file (TTF/OTF)
        #[arg(short, long)]
        font: String,

        /// Output PNG file path
        #[arg(short, long)]
        output_png: String,

        /// Output JSON metadata file path
        #[arg(short = 'j', long)]
        output_json: String,

        /// Glyph size in pixels (default: 64)
        #[arg(long, default_value = "64")]
        glyph_size: u32,

        /// Padding around each glyph (default: 8)
        #[arg(long, default_value = "8")]
        padding: u32,

        /// Characters to include (default: ASCII printable 32-126)
        #[arg(long)]
        charset: Option<String>,
    },

    /// Analyze text rendering quality (blur vs jaggedness)
    TextQuality {
        /// Reference image (with good text quality)
        #[arg(short, long)]
        reference: String,

        /// Current image to analyze
        #[arg(short, long)]
        current: String,

        /// Region to crop: x,y,width,height (e.g., "100,0,200,80")
        #[arg(long)]
        region: Option<String>,

        /// Save cropped images (prefix path)
        #[arg(long)]
        output_crop: Option<String>,
    },

    /// Analyze text quality of a single image (no reference)
    TextQualitySingle {
        /// Image to analyze
        #[arg(short, long)]
        image: String,

        /// Region to crop: x,y,width,height (e.g., "100,0,200,80")
        #[arg(long)]
        region: Option<String>,
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

        Commands::ExtractDomCdp {
            url,
            output,
            width,
            height,
        } => {
            commands::extract_dom_cdp::run(&url, &output, width, height)?;
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
            cdp_port,
        } => {
            commands::screenshot::run(&url, &output, width, height, true, cdp_port)?;
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

        Commands::PixelDiffSimple {
            reference,
            candidate,
            threshold,
        } => {
            commands::pixel_diff::run_simple(&reference, &candidate, threshold)?;
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
                eprintln!(
                    "   This occurs when Chrome 141+ sends CDP messages not yet in the library."
                );
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
                            let path = "classic/screenshots/screenshot.png";
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

        Commands::WasmStart {
            release,
            open,
            port,
            renderer,
        } => {
            commands::wasm_start::run(release, open, port, &renderer)?;
        }

        Commands::IntegrationTest { url } => {
            commands::integration_test::run(&url)?;
        }

        Commands::CaptureReference {
            file,
            out,
            headed,
            chrome_path,
            layout_json,
        } => {
            tokio::runtime::Runtime::new()?.block_on(async {
                commands::capture::run_capture_reference(
                    std::path::Path::new(&file),
                    std::path::Path::new(&out),
                    headed,
                    chrome_path.as_deref(),
                    layout_json.as_deref(),
                )
                .await
            })?;
            println!("✓ Saved: {}", out);
        }

        Commands::CaptureRenderer {
            url,
            out,
            headed,
            chrome_path,
        } => {
            tokio::runtime::Runtime::new()?.block_on(async {
                commands::capture::run_capture_renderer(
                    &url,
                    std::path::Path::new(&out),
                    headed,
                    chrome_path.as_deref(),
                )
                .await
            })?;
            println!("✓ Saved: {}", out);
        }

        Commands::DiffLayouts {
            left,
            right,
            threshold,
        } => {
            commands::capture::run_diff_layouts(
                std::path::Path::new(&left),
                std::path::Path::new(&right),
                threshold,
            )?;
        }

        Commands::GenerateMtsdfAtlas {
            font,
            output_png,
            output_json,
            glyph_size,
            padding,
            sdf_range,
            charset,
        } => {
            let config = commands::generate_mtsdf_atlas::AtlasConfig {
                glyph_size,
                padding,
                sdf_range,
                charset: charset.unwrap_or_else(|| (32u8..=126u8).map(|c| c as char).collect()),
            };
            commands::generate_mtsdf_atlas::run(&font, &output_png, &output_json, config)?;
        }

        Commands::GenerateSdfAtlas {
            font,
            output_png,
            output_json,
            glyph_size,
            padding,
            charset,
        } => {
            let config = commands::generate_sdf_atlas::AtlasConfig {
                glyph_size,
                padding,
                charset: charset.unwrap_or_else(|| (32u8..=126u8).map(|c| c as char).collect()),
            };
            commands::generate_sdf_atlas::run(&font, &output_png, &output_json, config)?;
        }

        Commands::TextQuality {
            reference,
            current,
            region,
            output_crop,
        } => {
            let region_tuple = region.map(|r| {
                let parts: Vec<u32> = r.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if parts.len() == 4 {
                    (parts[0], parts[1], parts[2], parts[3])
                } else {
                    panic!("Region must be: x,y,width,height");
                }
            });
            let metrics = commands::text_quality::run(
                &reference,
                &current,
                region_tuple,
                output_crop.as_deref(),
            )?;
            metrics.print_report();
        }

        Commands::TextQualitySingle { image, region } => {
            let region_tuple = region.map(|r| {
                let parts: Vec<u32> = r.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if parts.len() == 4 {
                    (parts[0], parts[1], parts[2], parts[3])
                } else {
                    panic!("Region must be: x,y,width,height");
                }
            });
            let metrics = commands::text_quality::run_single(&image, region_tuple)?;
            metrics.print_report();
        }
    }

    Ok(())
}
