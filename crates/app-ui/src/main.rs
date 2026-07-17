//! VsTerm — cross-platform SSH terminal manager.

// Release builds are a GUI app: do not allocate a console window on Windows.
// Keep the console in debug so `cargo run` still shows tracing logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod conn_error;
mod dialog_chrome;
mod remote_host;
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
            ..Default::default()
        },
        ..Default::default()
    };

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
    setup.power_preference = wgpu::PowerPreference::HighPerformance;
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
        let hardware = adapters.iter().find(|a| {
            let ty = a.get_info().device_type;
            matches!(
                ty,
                wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu
            ) && surface_ok(a, surface)
        });
        if let Some(adapter) = hardware {
            return Ok(adapter.clone());
        }
        let any_non_cpu = adapters.iter().find(|a| {
            a.get_info().device_type != wgpu::DeviceType::Cpu && surface_ok(a, surface)
        });
        if let Some(adapter) = any_non_cpu {
            tracing::warn!(
                "using fallback wgpu adapter (no discrete/integrated GPU): {}",
                adapter.get_info().name
            );
            return Ok(adapter.clone());
        }
        if cfg!(debug_assertions) {
            if let Some(adapter) = adapters.iter().find(|a| surface_ok(a, surface)) {
                tracing::warn!(
                    "wgpu using CPU software renderer ({}) — expect high idle CPU; \
                     update GPU drivers or avoid remote-desktop WARP",
                    adapter.get_info().name
                );
                return Ok(adapter.clone());
            }
        }
        Err(
            "no hardware GPU adapter — install/update graphics drivers (DX12). \
             VsTerm refuses CPU software rendering in release builds"
                .into(),
        )
    }));
    setup
}

fn surface_ok(adapter: &wgpu::Adapter, surface: Option<&wgpu::Surface<'_>>) -> bool {
    match surface {
        Some(surface) => adapter.is_surface_supported(surface),
        None => true,
    }
}
