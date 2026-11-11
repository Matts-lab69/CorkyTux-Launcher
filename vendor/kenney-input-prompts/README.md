# Kenney Input Prompts originals

`originals/` preserves the14 SVGs and CC0 license from source commit
`248fb87afc61a521b974696e2e5273649c70b6b1` byte for byte. The selected SVGs
also match the Generic, PlayStation Series, Nintendo Switch and Xbox Series
Vector files in the original Kenney Input Prompts1.5 collection.

Runtime assets in `assets/input-prompts/` are derived copies. Their only SVG
changes are adding `viewBox="0 0 64 64"` to the root element after its existing
height="64" attribute and adding a terminal newline. Geometry, colors,
dimensions and all other bytes are unchanged. The license is copied unchanged
alongside the runtime assets; originals retain their original line endings.

Keep originals outside `assets/`, which contains only shipped artwork and is
covered by the unchanged `every_svg_declares_a_viewbox` contract. Do not add
scanner exceptions or normalize the originals in place.

This independent recovery batch contains assets and their byte-preservation
tests only. It does not enable the old controller polling loop or change the
visible prompt/help UI; those runtime changes are outside this asset contract.
