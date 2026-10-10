# Rebuild rung-ui-design-v1

Record: `../rung-ui-design-v1.pdf` (the committed PDF is the reviewed copy; a copy also sits in `~/Documents/Rung/`).

    ./make.sh                 # builds build/deck.pdf (+ build/png/, ignored)
    ./make.sh --publish DIR   # also copies to DIR/rung-ui-design-v1.pdf

The deck embeds the mockup captures in `mockups/shots/`. To remake them (only when a mockup page changes):

    cd mockups
    python3 gen.py            # writes the six static pages (invented data) from gen.py + app.css
    node capture.mjs          # 12 captures: 390x844 at 2x, and 1920 wide (first screen)
    node census.mjs           # the element and word census; exits non-zero on a failure

`census-first-draft.txt` is the census run on the first draft of the pages (it fails on four counts); `census-tap-first.txt` is the run after the tap-target check was widened to inputs and labels (it failed on the Configure checkbox at 390, whose hit area was 24 pixels); `census-final.txt` is the run on the pages as committed (all pass). The budgets are in the header of `census.mjs`.

Needs: python3, node, Playwright 1.62.1 (mise `npm-playwright`, or set `PLAYWRIGHT_MODULE`), fonts Noto Sans / Liberation Sans. Shared pieces (playwright.mjs, which finds Playwright for render.mjs and the mockup scripts; render.mjs, theme.css, slide helpers, make.sh) live in `../_build/`; this directory holds the deck's content (`build.py`, `diagram.svg`), the mockups, and a thin `make.sh`. `build.py` ends with a text privacy scan that fails the build on a forbidden term.
