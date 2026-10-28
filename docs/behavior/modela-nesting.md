# Modela Nesting

`Window > Panels > Modela Nesting` arranges selected page objects on the current
page. It is kept in `apps/tracedraw/src/modela_nesting/` so the placement
engine and its interface can be maintained separately from the editor.

## Inputs

- Width starts with the current page width. The field uses the document's
  current units, and the current page is resized to this width when nesting.
- Spacing uses the document's current units and is the minimum distance between
  the visual bounding boxes of two objects.
- Allow rotation lets the engine compare the original orientation with a
  90-degree rotation.

Groups and free objects are accepted. Objects selected inside a group are not,
because their parent is not one of the pieces being arranged. A group is
transformed as one object, so all of its children, styles and effects stay
together.

## Placement

The first implementation uses the visual bounding rectangle of each object. It
places larger objects first and searches positions at the right and top edges of
the objects already placed. The chosen position minimizes the used material
length, then prefers the lowest and leftmost position. When rotation is
allowed, both orientations take part in that comparison.

The page height becomes the material length used by the result. Resizing the
page and transforming every object form one undo step named `Modela Nesting`.
This module boundary is also the replacement point for Modela's contour-based
polygon engine.
