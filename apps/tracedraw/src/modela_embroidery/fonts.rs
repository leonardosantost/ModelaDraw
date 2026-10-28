//! The embroidery font library: the digitized fonts lettering can use, and
//! their download.
//!
//! The library is a zip on the project's releases (`embroidery-fonts`
//! tag): the fonts whose licences let everyone use and share them, each
//! with its licence, and `FONTS.md` with the credits. Download Embroidery
//! Fonts unpacks it into the user's font folder (`embroidery-fonts` in the
//! preferences folder), in the background, and the font list reads it
//! again. The browser build has no font folder.

use std::sync::Mutex;
use tracedraw_emb_fonts::library;

/// Where the font library is published.
pub const FONTS_URL: &str = "https://github.com/leonardosantost/ModelaDraw/releases/download/embroidery-fonts/modeladraw-embroidery-fonts.zip";
/// The folder at the top of the zip.
const ZIP_TOP: &str = "embroidery-fonts";
/// Most bytes the download and the unpacked fonts may take.
const MAX_BYTES: u64 = 512 << 20;

/// Where the download is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Download {
    #[default]
    Idle,
    /// Bytes received so far.
    Running(u64),
    /// Done: the fonts the library now has.
    Done(usize),
    Failed(String),
}

static STATE: Mutex<Download> = Mutex::new(Download::Idle);

fn set(d: Download) {
    if let Ok(mut s) = STATE.lock() {
        *s = d;
    }
}

/// Where the download is now.
pub fn state() -> Download {
    STATE.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Whether this build can download fonts (not the browser).
pub fn can_download() -> bool {
    cfg!(not(target_arch = "wasm32")) && library::user_folder().is_some()
}

/// The fonts the library has, in name order.
pub fn installed() -> Vec<library::Entry> {
    library::entries()
}

/// Start downloading the font library (nothing when it is already
/// running); `ctx` is asked to repaint as it goes.
pub fn start(ctx: &egui::Context) {
    if matches!(state(), Download::Running(_)) {
        return;
    }
    set(Download::Running(0));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = run(&ctx);
            set(match result {
                Ok(n) => Download::Done(n),
                Err(e) => {
                    log::warn!("embroidery fonts: {e}");
                    Download::Failed(e)
                }
            });
            ctx.request_repaint();
        });
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        set(Download::Failed("not in the browser".into()));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(ctx: &egui::Context) -> Result<usize, String> {
    use std::io::Read;
    let folder = library::user_folder().ok_or("no preferences folder")?;
    let parent = folder
        .parent()
        .ok_or("no preferences folder")?
        .to_path_buf();
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let mut body = crate::updates::download(FONTS_URL)?;
    let mut bytes: Vec<u8> = vec![];
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = body.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(buf.get(..n).unwrap_or_default());
        if bytes.len() as u64 > MAX_BYTES {
            return Err("the font library is larger than expected".into());
        }
        set(Download::Running(bytes.len() as u64));
        ctx.request_repaint();
    }
    // Unpacked beside the folder, then put in its place: a failed download
    // leaves the fonts there were.
    let part = parent.join("embroidery-fonts.part");
    let _ = std::fs::remove_dir_all(&part);
    unpack(&bytes, &part)?;
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::rename(&part, &folder).map_err(|e| e.to_string())?;
    library::rescan();
    Ok(library::entries().len())
}

/// The files of the zip `bytes` under its top folder, into `to`.
#[cfg(not(target_arch = "wasm32"))]
fn unpack(bytes: &[u8], to: &std::path::Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        // Only names that stay inside the folder.
        let Some(name) = f.enclosed_name() else {
            continue;
        };
        let Ok(rel) = name.strip_prefix(ZIP_TOP) else {
            continue;
        };
        let out = to.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        total += f.size();
        if total > MAX_BYTES {
            return Err("the font library is larger than expected".into());
        }
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_zip_unpacks_its_folder_and_nothing_outside_it() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opt: zip::write::SimpleFileOptions = Default::default();
            z.start_file("embroidery-fonts/FONTS.md", opt).unwrap();
            z.write_all(b"credits").unwrap();
            z.start_file("embroidery-fonts/abc/font.json", opt).unwrap();
            z.write_all(b"{}").unwrap();
            z.start_file("../evil.txt", opt).unwrap();
            z.write_all(b"no").unwrap();
            z.start_file("other/x.txt", opt).unwrap();
            z.write_all(b"no").unwrap();
            z.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("modeladraw-fonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        unpack(buf.get_ref(), &dir).unwrap();
        assert!(dir.join("FONTS.md").is_file());
        assert!(dir.join("abc/font.json").is_file());
        assert!(!dir.join("x.txt").exists() && !dir.join("other").exists());
        assert!(!dir.parent().unwrap().join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The real download from the release, into the preferences folder:
    /// run with `--ignored`.
    #[test]
    #[ignore]
    fn live_download_installs_the_library() {
        let ctx = egui::Context::default();
        let n = run(&ctx).unwrap();
        println!("{n} fonts in {:?}", library::user_folder());
        assert!(n >= 50);
        assert!(library::user_folder().unwrap().join("FONTS.md").is_file());
    }
}
