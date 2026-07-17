//! VsTerm — cross-platform SSH terminal manager.

// Release builds are a GUI app: do not allocate a console window on Windows.
// Keep the console in debug so `cargo run` still shows tracing logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod conn_error;
mod dialog_chrome;
mod remote_host;
mod render_policy;
mod fonts;
mod fx;
mod icon;
mod i18n;
mod metrics;
mod os_icon;
mod panels;
mod term_highlight;
mod terminal_view;
mod sys_file_icon;
mod theme;
mod ui_icon;
mod ctx_menu;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

// SFTP transfers churn packet buffers across russh and disk-writer threads.
// mimalloc handles cross-thread frees without serializing the UI heap.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> Result<()> {
    // Used as SSH_ASKPASS helper when collecting remote metrics / routes.
    // Works with windows_subsystem: OpenSSH pipes stdout; no console is needed.
    if std::env::var_os("VSTERM_ASKPASS_MODE").is_some() {
        if let Ok(secret) = std::env::var("VSTERM_ASKPASS_SECRET") {
            use std::io::Write;
            print!("{secret}");
            let _ = std::io::stdout().flush();
        }
        std::process::exit(0);
    }

    // Default quieter in release GUI builds; override with RUST_LOG if needed.
    let default_filter = if cfg!(debug_assertions) {
        "info"
    } else {
        "warn"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .with_ansi(cfg!(debug_assertions))
        .init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1400.0, 860.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true)
        .with_title("VsTerm");
    if let Ok(icon) = icon::window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(wgpu_setup()),
            // Repaint cadence is controlled by render_policy. Fifo/AutoVsync
            // makes DX12 WARP busy-wait across several worker threads after a
            // restored window, so it is unsuitable for supported RDP/VM use.
            present_mode: wgpu::PresentMode::AutoNoVsync,
            // One buffered frame is enough for a UI; default latency keeps more
            // swapchain images resident and inflates working set.
            desired_maximum_frame_latency: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };

    tracing::info!("wgpu present_mode=AutoNoVsync (application-controlled cadence)");

    eframe::run_native(
        "VsTerm",
        native_options,
        Box::new(|cc| Ok(Box::new(app::VsTermApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

fn preferred_backends() -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    {
        wgpu::Backends::DX12
    }
    #[cfg(target_os = "macos")]
    {
        wgpu::Backends::METAL
    }
    #[cfg(target_os = "linux")]
    {
        wgpu::Backends::VULKAN
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))]
    {
        wgpu::Backends::PRIMARY
    }
}

fn wgpu_setup() -> eframe::egui_wgpu::WgpuSetupCreateNew {
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::default();
    setup.instance_descriptor.backends = preferred_backends();
    // Prefer integrated GPU when available — discrete adapters often reserve
    // far more driver/working-set memory for an egui UI than we need.
    setup.power_preference = wgpu::PowerPreference::LowPower;
    setup.device_descriptor = std::sync::Arc::new(|adapter| {
        let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits::downlevel_webgl2_defaults()
        } else {
            wgpu::Limits::default()
        };
        wgpu::DeviceDescriptor {
            label: Some("egui wgpu device"),
            required_features: wgpu::Features::default(),
            required_limits: wgpu::Limits {
                max_texture_dimension_2d: 8192,
                ..base_limits
            },
            memory_hints: wgpu::MemoryHints::MemoryUsage,
        }
    });
    setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters, surface| {
        if adapters.is_empty() {
            return Err(
                "no wgpu adapters — check GPU drivers / backend (DX12/Metal/Vulkan)".into(),
            );
        }
        for adapter in adapters {
            let info = adapter.get_info();
            tracing::info!(
                "wgpu adapter candidate: {} ({:?}, {:?})",
                info.name,
                info.backend,
                info.device_type
            );
        }
        // Integrated first (lower RSS), then discrete — still skip CPU unless
        // nothing else can present to the surface.
        let pick = |ty: wgpu::DeviceType| {
            adapters.iter().find(|a| {
                a.get_info().device_type == ty && surface_ok(a, surface)
            })
        };
        if let Some(adapter) = pick(wgpu::DeviceType::IntegratedGpu) {
            render_policy::set_software_renderer(false);
            tracing::info!(
                "wgpu selected integrated GPU: {}",
                adapter.get_info().name
            );
            return Ok(adapter.clone());
        }
        if let Some(adapter) = pick(wgpu::DeviceType::DiscreteGpu) {
            render_policy::set_software_renderer(false);
            tracing::info!(
                "wgpu selected discrete GPU: {}",
                adapter.get_info().name
            );
            return Ok(adapter.clone());
        }
        let any_non_cpu = adapters.iter().find(|a| {
            a.get_info().device_type != wgpu::DeviceType::Cpu && surface_ok(a, surface)
        });
        if let Some(adapter) = any_non_cpu {
            render_policy::set_software_renderer(false);
            tracing::warn!(
                "using fallback wgpu adapter (no discrete/integrated GPU): {}",
                adapter.get_info().name
            );
            return Ok(adapter.clone());
        }
        if let Some(adapter) = adapters.iter().find(|a| surface_ok(a, surface)) {
            render_policy::set_software_renderer(true);
            tracing::warn!(
                "wgpu using supported CPU software renderer ({}) with reduced animation cadence",
                adapter.get_info().name
            );
            return Ok(adapter.clone());
        }
        Err("no wgpu adapter supports the application surface".into())
    }));
    setup
}

fn surface_ok(adapter: &wgpu::Adapter, surface: Option<&wgpu::Surface<'_>>) -> bool {
    match surface {
        Some(surface) => adapter.is_surface_supported(surface),
        None => true,
    }
}
