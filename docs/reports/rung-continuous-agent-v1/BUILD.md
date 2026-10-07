# Rebuild rung-continuous-agent-v1

Record: `../rung-continuous-agent-v1.pdf` (the committed PDF is the reviewed copy; a copy also sits in `~/Documents/Rung/`).

    ./make.sh                 # builds build/deck.pdf (+ build/png/, ignored)
    ./make.sh --publish DIR   # also copies to DIR/rung-continuous-agent-v1.pdf

Needs: python3, node, Playwright 1.62.1 (mise `npm-playwright`, or set `PLAYWRIGHT_MODULE`), fonts Noto Sans / Liberation Sans. Shared pieces (render.mjs, theme.css, slide helpers, make.sh) live in `../_build/`; this directory holds only the deck's content (`build.py`, SVGs) and a thin `make.sh`. All paths are relative. `build.py` ends with a text privacy scan that fails the build on a forbidden term.
