<p align="center">
  <img src="assets/icon/tracedraw-256.png" alt="ModelaDraw icon" width="128" height="128">
</p>

<h1 align="center">ModelaDraw</h1>

<p align="center">
  <b>Vector design, pattern making and embroidery in one open-source program.</b><br>
  <sub><i>powered by <a href="https://github.com/igtoth/tracedraw">TraceDraw</a></i></sub>
</p>

<p align="center">
  Draw it, cut it, stitch it: a full vector editor with pattern pieces, grading and nesting for
  garments, and an embroidery workspace that turns your art into machine files,<br>
  natively on Windows, macOS and Linux, and in the browser.
</p>

<p align="center">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-b7410e?style=flat-square&logo=rust">
  <img alt="Windows, macOS, Linux and the browser" src="https://img.shields.io/badge/Windows%20%C2%B7%20macOS%20%C2%B7%20Linux%20%C2%B7%20Web-native-1f6fc5?style=flat-square">
  <img alt="Embroidery machine files" src="https://img.shields.io/badge/embroidery-DST%20%C2%B7%20PES%20%C2%B7%20JEF%20%C2%B7%20EXP%20%C2%B7%20VP3-16a34a?style=flat-square">
  <img alt="License: MIT or Apache-2.0" src="https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-444444?style=flat-square">
  <img alt="Status: alpha" src="https://img.shields.io/badge/status-alpha-d97706?style=flat-square">
  <img alt="18 interface languages" src="https://img.shields.io/badge/interface-18%20languages-7b5cff?style=flat-square">
</p>

<p align="center">
  <a href="https://github.com/leonardosantost/ModelaDraw/releases/latest"><b>Download</b></a> &middot;
  <a href="https://leonardosantost.github.io/ModelaDraw/"><b>Open it in the browser</b></a> &middot;
  <a href="docs/behavior/modela-embroidery.md">Embroidery guide</a> &middot;
  <a href="docs/features.md">Feature status</a>
</p>

<br>

<p align="center">
  <img src="docs/images/tracedraw-hero.jpg" alt="The editor with an illustrated poster of a lake at dusk: the title is selected, the property bar shows its font, size and alignment, and the Objects panel lists the five layers and their named objects" width="100%">
  <br><sub>The vector editor at the heart of ModelaDraw: pages, layers, curves, text and effects.</sub>
</p>

<p align="center">
  <a href="#three-workspaces-one-drawing">Workspaces</a> ·
  <a href="#embroidery">Embroidery</a> ·
  <a href="#pattern-making">Pattern making</a> ·
  <a href="#a-look-around">A look around</a> ·
  <a href="#why-modeladraw">Why ModelaDraw</a> ·
  <a href="#downloads">Downloads</a> ·
  <a href="#build-from-source">Build</a> ·
  <a href="#license-and-credits">License</a>
</p>

## Three workspaces, one drawing

Pick a workspace from the selector at the right of the menu bar (or **Window › Workspace**). Each
one opens the panels and bars for its job; the drawing stays the same file.

| Workspace | For | What opens |
|---|---|---|
| **Design** | Illustration, logos, print | The full vector editor: toolbox, property bar, Objects panel |
| **Pattern Making** | Garment patterns | Pattern piece tools, Properties, Grading and Nesting panels |
| **Embroidery** | Machine embroidery | Embroidery, Sewing Sequence and Hoop panels, the embroidery bar |

Every workspace bar moves like the other toolbars: dock it at the top or the bottom, or float it.

## Embroidery

Turn vectors into stitches only where you choose, see exactly what the machine will sew, and
export it.

- **Convert when you want.** A vector stays a vector until you convert it: **Convert to Satin**
  turns a closed shape into satin columns along its centre line, and a line into a satin along
  itself; or pick a fill (Tatami, Contour, Circular, Gradient, Motif, Guided) or line stitch
  (Single and Triple Run, Satin, E-Stitch, Rope) for any object. Drawings with thousands of
  vectors stay fast.
- **Three views.** Vector, Stitches and **Realistic** (each stitch drawn as lit thread over the
  fabric). The page is the hoop: a frame round the sewing field, and anything past it in red.
- **Clean joins.** Each object ends near where the next one starts, and moves between parts run
  as hidden travel under fills and satins sewn later, instead of loose threads across the design.
  **Apply Nearest Join** (`J`) keeps those joins, and with the Shape tool you drag each object's
  start (green square) and end (red cross) where you want them.
- **Sewing Sequence.** Every part in colour blocks: drag a row to reorder, click a swatch to
  change the thread. Sort by Color in one click.
- **Lettering** in a built-in satin font, editable in place.
- **Check Design** finds what would sew badly (too dense, too thin, too wide a satin, outside
  the hoop) and fixes it; **Remove Overlaps** cuts the hidden parts of covered fills; a density
  map and a stitch-by-stitch **sew-out player** show the result before it reaches the fabric.
- **Hoops, fabrics and thread charts:** brand hoops, eight fabric presets that set density,
  pull compensation and underlay, and the Brother and Janome thread charts.
- **Machine files:** **File › Export Machine File** writes DST, PES, JEF, EXP, VP3, XXX, U01
  and TAP; **File › Open Machine File** reads those and DSB, DSZ and HUS.

Details: [docs/behavior/modela-embroidery.md](docs/behavior/modela-embroidery.md).

<!-- Screenshots of the embroidery workspace go here (docs/images/modeladraw-embroidery-*.jpg). -->

## Pattern making

- **Pattern pieces** with seam allowance, grain line and direction, kept in the drawing itself.
- **Notches, Fold and Unfold** tools for the details of a piece.
- **Linked grading:** size definitions saved with each piece.
- **Nesting:** arrange the selected pieces on the material's width, with spacing and optional
  90° rotation, as short as it can be.

<!-- Screenshots of the pattern making workspace go here (docs/images/modeladraw-pattern-*.jpg). -->

## A look around

The vector editor ModelaDraw is built on, powered by TraceDraw.

<table>
<tr>
<td width="50%" valign="top">
  <img src="docs/images/tracedraw-curves.jpg" alt="Curves and nodes being edited with the Shape tool" width="100%">
  <p align="center"><sub><b>Curves and nodes</b>: real Bézier editing with the Shape tool</sub></p>
</td>
<td width="50%" valign="top">
  <img src="docs/images/tracedraw-fills.jpg" alt="Fountain, pattern and mesh fills on several objects" width="100%">
  <p align="center"><sub><b>Fills and outlines</b>: fountain, pattern, texture and mesh fills</sub></p>
</td>
</tr>
<tr>
<td width="50%" valign="top">
  <img src="docs/images/tracedraw-type.jpg" alt="Artistic and paragraph text with styles" width="100%">
  <p align="center"><sub><b>Type</b>: artistic and paragraph text, styles and frames</sub></p>
</td>
<td width="50%" valign="top">
  <img src="docs/images/tracedraw-trace.jpg" alt="A bitmap traced into vectors" width="100%">
  <p align="center"><sub><b>Trace</b>: turn a photo or a sketch into vectors, ready to stitch</sub></p>
</td>
</tr>
<tr>
<td width="50%" valign="top">
  <img src="docs/images/tracedraw-effects.jpg" alt="Live effects: blend, contour, envelope and drop shadow" width="100%">
  <p align="center"><sub><b>Live effects</b>: blend, contour, envelope, extrude and shadows</sub></p>
</td>
<td width="50%" valign="top">
  <img src="docs/images/tracedraw-export.jpg" alt="The export dialog with PDF, SVG and bitmap formats" width="100%">
  <p align="center"><sub><b>Files</b>: PDF/X, SVG, DXF, bitmaps and real <code>.cdr</code> files</sub></p>
</td>
</tr>
</table>

## Why ModelaDraw

- **One program for the whole job.** Design the art, cut the pattern, stitch the logo, without
  moving files between three applications.
- **Open.** Free and open source under MIT or Apache-2.0. Drawings save as `.tdraw`; it reads and
  writes SVG, PDF, DXF and bitmaps, opens `.cdr` files, and writes the embroidery formats machines
  read.
- **Fast and native.** Written in Rust; the stitch preview is worked out once per change and
  only drawn each frame.
- **Everywhere.** The same program on Windows, macOS and Linux, and in the browser with nothing
  to install.
- **Speaks your language.** The interface comes in 18 languages, Brazilian Portuguese and Spanish
  among them.

## Downloads

The [latest release](https://github.com/leonardosantost/ModelaDraw/releases/latest) has every
build below; [all releases](https://github.com/leonardosantost/ModelaDraw/releases) has the
earlier ones and their notes. `<version>` in the names is the version number. Every push to
`main` also refreshes a **Nightly build** pre-release.

| System | File | Notes |
|---|---|---|
| **Windows** (x64) | `modeladraw-<version>-windows-x86_64-setup.exe` | Installer: Start menu, desktop icon, `.tdraw` files |
| | `modeladraw-<version>-windows-x86_64-portable.zip` | Unzip and run; keeps its settings in its own folder |
| **macOS** (Apple silicon) | `modeladraw-<version>-macos-arm64.dmg` | Not notarized: the first time, right-click › Open |
| **macOS** (Intel) | `modeladraw-<version>-macos-x86_64.dmg` | Same |
| **Linux** (x86_64, ARM64) | `modeladraw-<version>-linux-amd64.AppImage`, `-arm64.AppImage` | Runs anywhere: `chmod +x` and start it |
| | `modeladraw_<version>_amd64.deb` | Debian and Ubuntu |
| | `modeladraw-<version>-linux-<arch>.tar.gz` | Unpack anywhere |
| **Web** | [leonardosantost.github.io/ModelaDraw](https://leonardosantost.github.io/ModelaDraw/) | No download; `modeladraw-<version>-web.zip` to host it yourself |

Windows may warn that the installer comes from an unknown publisher (it is not code-signed yet):
choose **More info › Run anyway**.

## Build from source

You need [Rust](https://rustup.rs) 1.97 or later.

```sh
git clone https://github.com/leonardosantost/ModelaDraw
cd ModelaDraw
cargo run --release -p tracedraw            # the desktop app (the program is called modeladraw)
cargo test -p tracedraw                     # the app's tests
scripts/web/build.sh                        # the browser version, into ./site
```

On Linux, install the window system's libraries first:
`sudo apt-get install pkg-config libxkbcommon-dev libwayland-dev libgl1-mesa-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgtk-3-dev`.

### Releases

The [build workflow](.github/workflows/build.yml) compiles and packages every system on GitHub's
machines. To publish a version:

1. Put the version in `Cargo.toml` (`version = "x.y.z"`) and a `## x.y.z (date)` section in
   [CHANGELOG.md](CHANGELOG.md) for its notes.
2. Push a tag `vx.y.z` (`git tag v0.20.0 && git push origin v0.20.0`), or run **Actions › build ›
   Run workflow** with the version.
3. The workflow builds the installers and attaches them to the release, and publishes the browser
   version on GitHub Pages.

## License and credits

ModelaDraw is free software under the [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
license, at your choice.

It is built on **[TraceDraw](https://github.com/igtoth/tracedraw)**, the open-source vector
editor by Ighor Toth and the TraceDraw contributors, which supplies the editor, the file formats
and the rendering. The embroidery stitch generators, stitch plan and machine formats are adapted
from an MIT OR Apache-2.0 embroidery project. Every bundled font, icon and asset, and every
license notice, is listed in [ATTRIBUTION.md](ATTRIBUTION.md).

<sub>ModelaDraw is an independent open-source project. Trademarks and product names belong to
their owners; file format names such as <code>.cdr</code>, <code>.dst</code> and <code>.pes</code>
are used only to describe the files ModelaDraw reads and writes. The software is provided "as
is", without warranty of any kind, as the licenses state.</sub>
