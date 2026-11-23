# MURB report page — design

Date: 2026-11-23. Status: approved in conversation ("Build it into makit"), calls stated there.

## Intent

`murb-simulate` writes, beside its results JSON, a self-contained HTML page like
the "Ifc4_ARC in Toronto" artifact an earlier session built by hand: the glass on a
compass next to the three intensities (with the difference against the same
building with its glass spread evenly), the year as heating above and cooling
below a line, the envelope as a symbolic section with its figures, and notes on
how the numbers were made. `murb-report --format html` regenerates it from a
results file.

## Calls made

- **The simulation produces it.** `murb-simulate` writes `<output stem>.html`
  beside `murb_results.json` every run; `murb-report --format html [--output]`
  rebuilds it from a results JSON.
- **A second run gives the comparison.** When the glazing split is not the even
  25 % × 4, `murb-simulate` runs murb once more with the even split (no HTML
  report, name suffixed "(even glass)") and stores its intensities and monthly
  series as `even_glass` in the results. The page shows TEDI/TEUI/GHGI against it.
  An even split, or a failed comparison run, means no comparison on the page.
- **The results JSON carries what the page needs**, all optional so older files
  still load: `input` (the `MurbInput` sent), `envelope` (the model's envelope
  when a model was used), `model` (its path), `even_glass`, `command` (the makit
  command that reproduces the run, `-o key=value` sorted by key).
- **Self-contained.** Inline CSS and JS, system font stacks, no network; dark
  and light by `prefers-color-scheme`. The data is embedded as a JSON script
  (`<script id="makit-data" type="application/json">`, with `</` escaped) and
  the page draws from it; the title is HTML-escaped.
- **Derived in Rust, drawn in JS.** `murb_report::page_data(&MurbResults)`
  computes everything the page states (title without murb's `_TMY`/`_ISD`
  suffix, facings biggest first with shares and compass names, monthly MWh,
  totals, the envelope ratios, the notes, the lede sentence) so it is tested in
  Rust; the JS only lays it out.
- **Without a model** (typed areas, or an old results file) the page still
  renders: the compass is empty or from the typed split, the floor is the GFA,
  and a note says the areas were typed or not recorded.

## Page

1. Header: eyebrow "makit · murb-simulate", the run name, one sentence (storeys,
   GFA, weather, where the areas came from).
2. "Where the glass faces · what it costs": compass (beam length ∝ area) and
   TEDI as the hero figure with "−x vs y if the glass faced all four ways
   equally", then TEUI and GHGI with their deltas; a lede sentence on the
   dominant facing and the heating difference.
3. "The year": heating above / cooling below bars in MWh with hover, the two
   peaks labelled, totals (with the even-glass totals in brackets), a table.
4. "The envelope": section glyph with roof, glass, walls above and below grade,
   floor over N levels; ratios: envelope above grade to floor, glass share of
   the above-grade wall.
5. "How these numbers were made": floor plates per storey and what was left
   out, what counts as glass, the grade split, U-values and COPs (noting makit's
   defaults), that murb is written for MURBs, the reproducing command.

## Tests

Rust: `page_data` on a results fixture shaped like the Revit house (title,
facings, deltas, MWh, ratios, notes, command); `render_html` self-contained
with parseable embedded data; a name with markup cannot break the page; old
results without a model still render; `page_path_for`. `murb.rs`:
`even_glass_input` is `None` for the even split; `command_line` sorts and
quotes; the new result fields round-trip and default to `None`. End to end on
the Revit house, opened in the browser.
