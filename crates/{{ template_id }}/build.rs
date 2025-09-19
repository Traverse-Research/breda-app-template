fn main() {
    println!("cargo:rerun-if-changed=../../locales");
    println!("cargo:rerun-if-changed=assets/credits");
    println!("cargo:rerun-if-changed=assets/license.md");

    cfg_aliases::cfg_aliases! {
        // https://github.com/Traverse-Research/steamworks-rs/blob/a95e3ea94acefa45a780bcc8260ddc0f094d2b76/steamworks-sys/build.rs#L24
        steam_supported: { any(
                all(target_os = "windows", target_arch = "x86_64"),
                target_os = "linux",
                target_os = "darwin"
            )
        },
    }

    let default_icon_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/resources/windows/icon.ico");

    let mut win_res =
        breda_build::windows_resource::WindowsResource::new(&breda_build::ProductInfo::default());
    win_res.set_icon(default_icon_path);
    win_res.set_manifest(include_str!("assets/resources/windows/evolve.manifest"));

    breda_build::Context::new()
        .windows_resource(win_res)
        .build();
}
