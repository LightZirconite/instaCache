//! Compiles the few C++ calls this project cannot make from Rust, and on
//! Windows puts the icon inside the executable.
//!
//! `QGuiApplication::setDesktopFileName` is what a Wayland compositor reads to
//! decide which desktop entry a window belongs to, and therefore which icon
//! the task bar draws. No Rust binding exposes it, and nothing else in Qt's
//! API can stand in: the Wayland plugin calls `desktopFileName()` directly and
//! passes the result to `xdg_toplevel::set_app_id`. On Windows the same C++
//! sets a site's window icon and draws the unread count on the task bar
//! button (`badge.rs`).
//!
//! The Qt include path and the compiler flags Qt needs — C++17 everywhere,
//! and MSVC's `/Zc:__cplusplus`, without which Qt 6's headers refuse to
//! compile — come from `qttypes`, which resolves them with qmake and
//! publishes them to dependent build scripts. That is why `qttypes` is a
//! direct dependency even though the code uses it only for `QString`.

fn main() {
    let qt_include_path = std::env::var("DEP_QT_INCLUDE_PATH")
        .expect("qttypes did not publish the Qt include path; is qmake6 on PATH?");
    let flags = std::env::var("DEP_QT_COMPILE_FLAGS").unwrap_or_default();

    let mut config = cpp_build::Config::new();
    config.include(&qt_include_path);
    for module in ["QtCore", "QtGui"] {
        config.include(format!("{qt_include_path}/{module}"));
    }
    for flag in flags.split_terminator(';') {
        config.flag(flag);
    }
    config.build("src/lib.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // What the task bar button's overlay needs from Windows itself.
        for library in ["ole32", "uuid", "user32"] {
            println!("cargo:rustc-link-lib={library}");
        }
        // `IDI_ICON1` is the name Qt looks up for its default window icon,
        // and the first icon in the file is what Explorer shows for it.
        #[cfg(windows)]
        embed_resource::compile("assets/instacache.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("could not embed the icon into instacache.exe");
        println!("cargo:rerun-if-changed=assets/instacache.rc");
        println!("cargo:rerun-if-changed=assets/instacache.ico");
    }

    for source in ["src/lib.rs", "src/sites.rs", "src/badge.rs"] {
        println!("cargo:rerun-if-changed={source}");
    }
    println!("cargo:rerun-if-env-changed=DEP_QT_INCLUDE_PATH");
}
