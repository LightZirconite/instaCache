//! Compiles the one C++ call this project cannot make from Rust.
//!
//! `QGuiApplication::setDesktopFileName` is what a Wayland compositor reads to
//! decide which desktop entry a window belongs to, and therefore which icon
//! the task bar draws. No Rust binding exposes it, and nothing else in Qt's
//! API can stand in: the Wayland plugin calls `desktopFileName()` directly and
//! passes the result to `xdg_toplevel::set_app_id`.
//!
//! The Qt include path comes from `qttypes`, which resolves it with qmake and
//! publishes it to dependent build scripts. That is why `qttypes` is a direct
//! dependency even though the code uses it only for `QString`.

fn main() {
    let qt_include_path = std::env::var("DEP_QT_INCLUDE_PATH")
        .expect("qttypes did not publish the Qt include path; is qmake6 on PATH?");

    let mut config = cpp_build::Config::new();
    config.include(&qt_include_path);
    for module in ["QtCore", "QtGui"] {
        config.include(format!("{qt_include_path}/{module}"));
    }
    config.flag_if_supported("-std=c++17");
    config.build("src/lib.rs");

    println!("cargo:rerun-if-changed=src/sites.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-env-changed=DEP_QT_INCLUDE_PATH");
}
