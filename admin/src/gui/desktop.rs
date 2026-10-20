//! On Linux, desktops show a window's icon through a launcher entry matching its app id. The
//! app adds one for the current user (and keeps it pointing at this program) when it starts.

pub const ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");

/// Add or update `~/.local/share/applications/ug-admin.desktop` and its icon. Failures are
/// ignored: it only affects the icon in the dock and app grid.
pub fn install_launcher() {
    #[cfg(target_os = "linux")]
    {
        // A development build would leave a launcher pointing into target/.
        if cfg!(debug_assertions) {
            return;
        }
        let _ = write_launcher();
    }
}

#[cfg(target_os = "linux")]
fn write_launcher() -> std::io::Result<()> {
    use std::fs;
    use std::path::PathBuf;

    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or(std::io::ErrorKind::NotFound)?;
    let exe = std::env::current_exe()?;
    let icon = data.join("icons/hicolor/256x256/apps/ug-admin.png");
    let entry = data.join("applications/ug-admin.desktop");
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=Ultimate Gamer Admin\n\
         Comment=Record The Ultimate Gamer tournaments and publish the results\n\
         Exec=\"{}\"\nIcon=ug-admin\nTerminal=false\nCategories=Game;Utility;\nStartupWMClass=ug-admin\n",
        exe.display()
    );
    for (path, bytes) in [(&icon, ICON_PNG), (&entry, desktop.as_bytes())] {
        if fs::read(path).ok().as_deref() != Some(bytes) {
            fs::create_dir_all(path.parent().expect("has a parent"))?;
            fs::write(path, bytes)?;
        }
    }
    Ok(())
}
