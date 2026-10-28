# Removing the background of a bitmap

[[menu.bitmaps > menu.bitmaps.remove_background_local]] makes the background
of a photo or scan transparent and keeps the subject. It works on this
computer and needs no AI service. The property bar of a selected bitmap has
the same command.

![The Remove Background dialog box](../../img/bitmaps-remove-background.png)

## To remove a background {#remove}

1. Select one or more bitmaps.
2. Click [[menu.bitmaps > menu.bitmaps.remove_background_local]]. The
   preview shows the bitmap before and after, with a checkerboard where it
   becomes transparent.
3. Set the options below until the subject stands out cleanly.
4. Click [[dialog.ok]]. Every selected bitmap changes in one undo step.

## Options {#options}

- **[[dialog.bg_colors]]** ModelaDraw learns the background's colors from the
  edges of the picture (up to four, so a gradient or a two-tone backdrop
  works). To choose them yourself, click the eyedropper and click the
  background in a preview; each click adds a color. Click a color swatch to
  take it out, and **[[dialog.bg_colors_auto_button]]** to go back to the
  learnt colors.
- **[[dialog.tolerance]]** How far a color may be from the background's and
  still count as background (0 to 100). Raise it when some backdrop stays;
  lower it when parts of the subject go.
- **[[dialog.edge_softness]]** Feathers the edge of the subject, in pixels.
  0 keeps a hard edge.
- **[[dialog.bg_contiguous]]** (on at first) Only the background connected to
  the edges of the picture goes, so a white shirt on a white backdrop keeps
  its inside. Turn it off to remove every pixel near a background color, for
  example the holes of a letter or a ring.
- **[[dialog.bg_despeckle]]** (on at first) Small loose spots left in the
  background go too.
- **[[dialog.bg_clean_edges]]** (on at first) Takes the old background's color
  out of the half-transparent pixels of the edge, so no light or dark halo
  shows when the subject goes on another background.
- **[[dialog.reset]]** puts every option back.

## Tips {#tips}

- It works best on a backdrop that differs from the subject: a studio
  background, a sky, a product on a plain table. For busy scenes, the AI
  command [[menu.bitmaps.remove_background]] (see
  [AI](../customize/ai.md)) separates subject and scene by their meaning.
- The result replaces the pixels; effects on the bitmap are made permanent.
  To fine-tune afterwards, use [Masking colors](bitmap-mask.md).
