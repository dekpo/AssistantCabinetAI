# embedding-load

Fictional only. `rapport-fictif-42-pages.pdf` is 42 pages of invented French-looking prose with a real text layer, about
114 000 characters and 126 chunks. It exists to put a realistic load on the embeddings path (`docs/TROUBLESHOOTING.md`,
"The AI stopped answering while indexing a long PDF") and is not part of the `gp-sandbox` set, so no inventory test counts it.

`make_pdf.py` regenerates it deterministically (`pip install reportlab`). Page 7 carries one sentence that appears
nowhere else (`MARKER` in the script); a log that contains it has stored document text.

To use it, copy the PDF into an empty work folder and press Analyse; do not put it in a folder that holds real documents.
