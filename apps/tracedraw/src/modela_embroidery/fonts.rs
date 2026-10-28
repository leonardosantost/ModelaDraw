//! The embroidery font library: the digitized fonts lettering can use, and
//! their download.
//!
//! The library is a zip on the project's releases (`embroidery-fonts`
//! tag): the fonts whose licences let everyone use and share them, each
//! with its licence, and `FONTS.md` with the credits. Download Embroidery
//! Fonts unpacks it into the user's font folder (`embroidery-fonts` in the
//! preferences folder), in the background, and the font list reads it
//! again. The browser build has no font folder.
//!
//! Reading the library and loading a font both take long (seconds for a
//! big font), so neither happens while a frame is drawn: the font list is
//! read once in the background and kept as names only, and a font is
//! loaded in the background when it is first chosen.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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

/// One font of the list: what the font picker shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// The font list: not read yet, being read, or read.
enum List {
    Unread,
    Reading,
    Read(Arc<Vec<Item>>),
}

static LIST: Mutex<List> = Mutex::new(List::Unread);

/// The library's fonts in name order, read in the background the first
/// time it is asked for (`None` until then); `ctx` repaints once it is.
pub fn installed(ctx: &egui::Context) -> Option<Arc<Vec<Item>>> {
    let Ok(mut list) = LIST.lock() else {
        return None;
    };
    match &*list {
        List::Read(items) => return Some(items.clone()),
        List::Reading => return None,
        List::Unread => *list = List::Reading,
    }
    drop(list);
    let read = || {
        let items: Vec<Item> = library::entries()
            .into_iter()
            .map(|e| Item {
                id: e.id,
                name: e.info.name,
                description: e.info.description,
            })
            .collect();
        if let Ok(mut list) = LIST.lock() {
            *list = List::Read(Arc::new(items));
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            read();
            ctx.request_repaint();
        });
        None
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        read();
        LIST.lock().ok().and_then(|l| match &*l {
            List::Read(items) => Some(items.clone()),
            _ => None,
        })
    }
}

/// Read the font list again (after a download).
fn reread() {
    library::rescan();
    if let Ok(mut list) = LIST.lock() {
        *list = List::Unread;
    }
    if let Ok(mut loads) = LOADS.lock() {
        *loads = None;
    }
}

/// Where loading a font is.
#[derive(Clone, Debug, PartialEq)]
pub enum Load {
    Loading,
    Ready,
    Failed(String),
}

static LOADS: Mutex<Option<HashMap<String, Load>>> = Mutex::new(None);

/// Font `id`, loaded in the background the first time it is asked for:
/// `Ready` once it can lay out lettering (the library keeps it).
pub fn load(ctx: &egui::Context, id: &str) -> Load {
    let Ok(mut loads) = LOADS.lock() else {
        return Load::Failed("the font cache is unavailable".into());
    };
    let loads = loads.get_or_insert_with(HashMap::new);
    if let Some(l) = loads.get(id) {
        return l.clone();
    }
    loads.insert(id.to_string(), Load::Loading);
    let id = id.to_string();
    let work = move || {
        let state = match library::load(&id) {
            Ok(_) => Load::Ready,
            Err(e) => Load::Failed(e),
        };
        if let Ok(mut loads) = LOADS.lock() {
            loads.get_or_insert_with(HashMap::new).insert(id, state);
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            work();
            ctx.request_repaint();
        });
        Load::Loading
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        work();
        Load::Ready
    }
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
    reread();
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

    /// How long the font list takes (the installed library): run with
    /// `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn time_the_font_list() {
        let ctx = egui::Context::default();
        let t = web_time::Instant::now();
        while installed(&ctx).is_none() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let scan = t.elapsed();
        let t = web_time::Instant::now();
        let mut n = 0;
        for _ in 0..1000 {
            n = installed(&ctx).map_or(0, |l| l.len());
        }
        println!(
            "{n} fonts: read in {scan:?} in the background, then {:?} a call",
            t.elapsed() / 1000
        );
    }

    /// How long each font takes to load: run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn time_font_loads() {
        let mut times: Vec<(std::time::Duration, String)> = vec![];
        for e in library::entries() {
            let t = web_time::Instant::now();
            let _ = library::load(&e.id);
            times.push((t.elapsed(), e.id.clone()));
        }
        times.sort();
        let total: std::time::Duration = times.iter().map(|t| t.0).sum();
        println!(
            "{} fonts, {total:?} in all; slowest {:?}",
            times.len(),
            &times[times.len().saturating_sub(5)..]
        );
    }
}
