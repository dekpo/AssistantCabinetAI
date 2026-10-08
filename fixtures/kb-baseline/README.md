# kb-baseline

Fictional only. A generator for the corpus that the Knowledge Base baseline is measured on
(`docs/test-reports/knowledge-base-pass-1/00-baseline.md`).

```text
python fixtures/kb-baseline/make_corpus.py <new-folder-outside-the-repository>
```

It writes `docs-10/`, `docs-30/` and `docs-50/` (50 short invented letters, the smaller folders being the first
10 and the first 30 of the same 50), `data/` (three semicolon-separated tables) and `ground-truth.txt`. The output is
deterministic: two runs give byte-identical folders, so two machines measure the same documents.

Why generated rather than committed: 50 letters are noise in the repository, and the point is that the owner
can regenerate them anywhere. Why these letters: every one carries the same letterhead and signature (real letters
do, and the analysis timings count byte-identical chunks), and one invented person appears in 2, 3 and 4 letters of the
10, 30 and 50 folders, so the entity question has a known answer at every size.

Keep the output outside the repository and outside any folder that a sync client mirrors. Never put `ground-truth.txt`
in a folder you analyse.
