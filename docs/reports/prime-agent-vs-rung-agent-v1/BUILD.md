# Rebuild prime-agent-vs-rung-agent-v1

Record: `../prime-agent-vs-rung-agent-v1.pdf` (the committed PDF is the reviewed copy; a copy also sits in `~/Documents/Rung/`).

    ./make.sh                 # builds build/deck.pdf (+ build/png/, ignored)
    ./make.sh --publish DIR   # also copies to DIR/prime-agent-vs-rung-agent-v1.pdf

Needs: python3, node, Playwright 1.62.1 (mise `npm-playwright`, or set `PLAYWRIGHT_MODULE`), fonts Noto Sans / Liberation Sans. Shared pieces (render.mjs, theme.css, slide helpers, make.sh) live in `../_build/`; this directory holds only the deck's content (`build.py`, SVGs) and a thin `make.sh`. All paths are relative. `build.py` ends with a text privacy scan that fails the build on a forbidden term.

Note: unlike the v3 deck, this deck's scan does not forbid file-name extensions in the PDF text, because the evidence index cites file paths and lines on purpose. All other forbidden terms still fail the build.
