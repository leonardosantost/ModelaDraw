# Modela Embroidery

The Embroidery workspace (the selector at the right of the menu bar, or
Window > Workspace > Embroidery) opens the Embroidery, Sewing Sequence and
Hoop panels and shows the embroidery bar. The bar moves like the other
toolbars: by its grip it docks in a row along the top or the bottom (beside
another toolbar or in a row of its own), or floats in a small window anywhere;
closed, it comes back from Window > Toolbars (or the bar menu), and Reset
Bar Positions puts it back along the top. The Pattern Making workspace's bar
moves the same way. Everything lives in
`apps/tracedraw/src/modela_embroidery/`; the stitch generators, the stitch
plan and the machine formats are the `tracedraw-emb-stitch`,
`tracedraw-emb-plan` and `tracedraw-emb-formats` crates in
`crates/embroidery/`.

## What sews

Nothing sews until it is converted: an object becomes embroidery when
Convert to Satin is used on it, a stitch type is chosen for one of its
parts in the Embroidery panel, or it is lettering. Every other object stays
a vector: it sews nothing, costs no stitch work (a drawing with many
vectors stays quick) and shows as a vector in every view. Remove Stitches
(in the Stitches section) makes a converted object a plain vector again.

The converted visible objects of the current page sew in paint order,
bottom to top; groups are opened and their objects sew in turn. Bitmaps,
tables, symbol instances and clip frames do not sew. Each object has two
parts:

- the fill sews first with a fill stitch (Tatami, Contour, Circular,
  Gradient, Motif or Guided fill) in the fill colour;
- the outline sews after it with a line stitch (Single Run, Triple Run,
  Satin, Satin Blocks, Manual, E-Stitch or Rope) in the outline colour.

"Sew the outline first" turns the order round. A part with no settings of
its own does not sew; the panel shows it with the fabric's defaults (Tatami
for a fill, a single run for an outline) until a type is chosen. A fountain, pattern or texture fill
sews in black. Satin Blocks and Manual are the whole object: the fill part
does not sew. A satin, E-stitch or rope outline is as wide as the outline,
and setting its width sets the outline width; choosing a running stitch
sets the outline to thread thickness (0.4 mm).

The settings are stored in the object's data under
`modela.embroidery.stitch` as JSON (`{fill?, stroke?, strokeFirst?}`, each
a set of stitch settings in millimetres and degrees), so `.tdraw` files
keep them without a change to the document format. Every change is one
undo step.

## The page is the hoop (Hoop panel)

The page's centre is where the needle starts, so a design sews where it
sits on the page. Picking a hoop in the Hoop panel sizes the page to the hoop's
sewing field; a page of another size is a custom hoop. Turn Hoop 90° swaps
the page's width and height. The panel also holds the fabric and the
thread chart. The panel says
whether the design fits (with a 0.15 mm allowance), fits only turned a
quarter turn, or is too large. Center in Hoop moves the selection (or
every unlocked object) to the middle of the page.

## Plan

Between objects the plan adds tie-ins and tie-offs (0.5 mm), trims before
moves longer than 3 mm, travel under fills sewn later instead of a jump,
colour changes, splits stitches longer than 12 mm and drops stitches under
0.3 mm. Sew time assumes 650 stitches a minute, 5 s a trim and 30 s a
colour change; thread use counts each stitch, 0.8 mm take-up a stitch and
15 mm tails, with the bobbin a third of the top thread. The density check
warns past 2.6 layers of thread in a 2 mm cell.

## Fabric and thread chart

The fabric (woven, knit, piqué, terry, fleece, denim, twill, leather) sets
the density, pull compensation and underlay of the stitches chosen for it. With a
thread chart (Brother or Janome), threads whose colour is in the chart are
named in machine files, and Match to Thread Chart moves the selection's
colours (or every object's) to the nearest chart thread (CIE Lab
distance). Fabric, chart, machine format and view options are remembered
between runs.

## Views

The bar's icon buttons switch between Vector (a pen nib), Stitches
(needle points) and Realistic (a spool); in the stitch views, icon toggles
show jumps and trims, underlay and the density map, and a play icon opens
the sew-out player. The tools are buttons with an icon and their name.
Each icon's tip names it. The Embroidery, Sewing Sequence and Hoop tabs
and the File menu's machine file rows have icons too.

In both stitch
views the page becomes fabric and the converted objects show as their
stitches only; the objects not converted show as the vectors they are,
under the stitches. Stitches draws every stitch as a line in
its thread colour (the underlay fainter; a jump with the thread cut first dashed grey, as it leaves no thread; one without a cut, which leaves the thread lying across the fabric, solid amber), Realistic
draws each as lit thread: a shadow down and right, the thread body and a
sheen down its middle, so white and black thread keep their shape.

In every view the page is drawn as the hoop: a frame 12 mm round the
sewing field, the field's edge dashed blue (red when stitches fall past
it), a round hoop's corners shaded, and the hoop's name. Stitches past the
field's edge (by more than 0.15 mm) are drawn in red over the artwork and
the stitches. The preview is worked out once for each
state of the drawing, page, fabric and chart, and only mapped to the screen
each frame.

## Tools

- Convert to Satin: each selected closed shape becomes Satin Blocks
  columns along its centre line (the exact medial axis on the desktop, a
  thinned skeleton in the browser), placed just above it in its colour; the
  shape is hidden, not removed. Over 10 mm wide, the status bar warns. A
  line (an outline without a fill) sews as a satin along itself, as wide as
  its outline, or the fabric's satin width when the outline is under 1 mm.
- Sort by Color: each layer's objects reordered so each thread colour sews
  in one go, colours in the order they first sew.
- Sewing Sequence panel: every sewn part in order, in colour blocks, with
  its thread, type and stitch count; a click selects the object. Dragging
  a row by its handle onto another sews it just before (upper half) or
  after (lower half) that one, by moving its top-level object next to the
  other's in the stacking order (objects in one group or on different
  layers are not moved, and the status bar says why). Clicking a swatch
  (a block's or a row's) opens a colour picker, applied with Apply, and
  the thread chart's threads, applied with a click: a fill part's fill or
  an outline part's outline takes the colour, in one undo step.

## Files

File > Export Machine File writes Tajima DST and TAP, Brother PES, Janome JEF,
Melco EXP, Husqvarna Viking VP3, Singer XXX or Barudan U01, the format
taken from the chosen file's extension (the last one used is offered first
and remembered). File > Open Machine File reads those and Barudan DSB, ZSK DSZ and Husqvarna Viking HUS onto the
current page, centred: each run of stitches is a manual-stitch path in its
thread colour, grouped.

## Joins, and where each object starts and ends

Each object starts near where the needle stopped and ends as near the next
object as it can: of the ways of sewing it with the same penetrations (a
tatami from either side, any object forwards or backwards) the one with the
shortest joins on both sides is taken. The next object's start is the one
placed on it by hand, or its point nearest this object. A closed outline
sews on along itself to its point nearest the next object.

Inside a fill, every move between its parts (a shape that splits into
parts, like a tree's trunk and crown) and from the underlay into the top
runs inside the shape as short travel stitches: straight where that stays
inside (or within 0.6 mm of the edge), else along the shape's own edge the
shorter way round. Only separate parts, or a way round more than six times
the straight distance plus 20 mm, keep a jump. After its first part, a fill
goes on with the part nearest the needle that leaves no unsewn row behind
it.

Between objects (and between separate parts of one), the move runs as
travel stitches under objects sewn later: straight when the whole line is
covered, else round inside one later object that both ends touch (within
0.6 mm). Fills and satin columns (the area between their rails) cover; a
move that no later object hides is a jump, with the thread cut first when
it is longer than 3 mm.

Apply Nearest Join (J, or the right-click menu with objects selected, in
the Embroidery workspace) keeps the starts and ends the shortest joins give
as the selected objects' own, in one undo step; Clear Sewing Start and End
takes them off again. With the Shape tool, each selected object that sews
shows its start as a green square and its end as a red cross; dragging one
drops it on the object's outline. A start inside a fill is reached by travel
inside it, sewn before its stitches cover it; an end, by travel over the
fill in its colour when the way stays inside. They are stored in the
object's data under `modela.embroidery.ends` (`{start?, end?}`, the
object's own space), so they move with it.

## Lettering

Add Lettering (the bar, or the Lettering section with nothing selected)
puts text across the middle of the page in the built-in centre-line font,
Modela Sans: each stroke of each letter sews as a satin column along its
centre line, the column 0.12 times the letter height wide (0.8 to 6 mm).
The object is a path placed by its transform, so it moves and turns like
any object. With one lettering object selected the section edits its text
(applied when the field is left: one undo step for the edit), letter
height (cap height, 2 to 300 mm), letter spacing (-50 to 100 mm) and line
spacing (0.8 to 5 times the height); the letters are laid out again where
they are. Characters the font does not have are left out and named in the
status bar.

The Font list (in the Lettering section, for new lettering with nothing
selected and for the selected lettering) has the built-in font and the
digitized embroidery fonts of the font library. Download Embroidery Fonts
(Update Embroidery Fonts once there are fonts) fetches the library, a zip
of about 45 MB on the project's `embroidery-fonts` release, in the
background, and unpacks it into `embroidery-fonts` in the preferences
folder (a failed download keeps the fonts there were); the list reads it
again. The library holds only the fonts whose licences let everyone use
and share them (SIL Open Font License, CC BY, CC BY-SA, public domain),
unchanged, each with its licence, and `FONTS.md` with the credits.
Fonts in the same format can also be put in that folder, or in a folder
named by `MODELADRAW_EMBROIDERY_FONTS`.

In a digitized font, lettering is a group of the font's pieces, each sewn
as the font digitized it (satin columns, fills, runs, with their own
direction and density), in the lettering's colour (the font's own colours
when it has several). Its settings are kept in the group's data under
`modela.embroidery.lettering`, so editing the text, size, spacing or font
lays the pieces out again inside the group; choosing the built-in font
turns it back into a path, and the other way round, in one undo step. New
lettering starts in the last font chosen, with a sample the font can sew
(the sample in capitals, or letters the font has). The browser version has
the built-in font only.

## Check Design

The Check Design section lists what will sew badly, errors first, and
rings each problem on the canvas (Mark on the drawing, on by default):

- past the hoop's edge (error; fix: Center in Hoop), or too large for it;
- spots where several objects stack more than 2.6 layers of thread in a
  2 mm cell (an error past 4; fix: Remove Overlaps when fills overlap);
- fills under 1.5 mm wide on average, twice the area over the outline
  length (fix: Convert to Satin);
- satins whose stitches are over 10 mm at the 90th percentile (fix: split
  long stitches) or under 1 mm at the median (fix: sew as a running
  stitch);
- objects whose stitches fit in 1.5 mm.

A click on a problem selects the objects involved; Fix runs its fix.

## Remove Overlaps

Each fill of the selection (every fill with nothing selected) is cut where
fills sewn after it cover it, keeping 1 mm under the covering shapes' edges
so no fabric shows; a fill covered completely and text are left as they
are. One undo step.

## Sew-out player and density map

In the stitch view, Sew-Out sews the design stitch by stitch: play and
pause, back to the start, one stitch on, a scrubber and 50 to 5000
stitches a second; a red ring marks the needle. Density colours each 2 mm
cell by its layers of thread: clear up to 1, green to 1.8, amber to 2.6,
red past it.

## Not yet

Digitized embroidery fonts and lettering on a path, the realistic thread
view, mockups, the production worksheet, the interactive Satin Blocks and
Fill Angle tools and guided-fill guides are not in this version.
