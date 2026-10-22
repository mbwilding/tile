use embed_manifest::{embed_manifest, manifest::DpiAwareness, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // Per-monitor DPI awareness, so window coordinates are physical pixels everywhere.
        embed_manifest(new_manifest("tiling").dpi_awareness(DpiAwareness::PerMonitorV2))
            .expect("unable to embed manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
