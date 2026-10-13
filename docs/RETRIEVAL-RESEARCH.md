# Retrieval research: every known way to serve the right passages to the model

13 October 2026 · research requested by the owner before sessions 1 and 2 (`docs/SESSION-RETRIEVAL-V2.md`, `docs/SESSION-LONG-DOCUMENTS.md`) · **nothing here is built.** Written for the owner and for the agent who builds it. English; interface strings are French.

**How to read the evidence.** Web sources found on 13 October 2026. Most are vendor or practitioner blogs that cite each other; the few primary sources are marked **(primary)**. A figure marked *(unverified)* could not be traced to its paper. Everything about this repository is read from the code or the study `docs/test-reports/knowledge-base-pass-1/07-record-study-fixes-and-freeze.md`. Nothing below was measured on her library yet: **the first step of the plan is to measure** (section 6).

## 1. The principle

The owner's priority: **the application must find what the user looks for without relying on the model.** That is achievable for *finding*; it is not for *phrasing*. So the design makes the retrieval result a **first-class output the user sees on its own** (the located, highlighted passages with section, page, author, date), and the model's answer a convenience written on top of it. Every method below is judged on three questions: does it raise **recall** (the right passage is among the candidates), does it raise **precision** (the right passage is at the top), and does it fit **our constraints** (local; free licence, Apache 2.0 or MIT; Windows and macOS; no hidden heavy work; French first).

## 2. Where the current system loses (read in the code)

- One embedding model, `nomic-embed-text` (`.env.example:16`, `models/LICENSES.md:38`, the Hugging Face card is v1.5). Third-party sources describe v1.5 as English-oriented and v2-moe as the multilingual version ([HF card](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5), [Docker listing of v2-moe](https://hub.docker.com/r/ai/nomic-embed-text-v2-moe)); the repository's own note calls it multilingual and names `bge-m3` as the French candidate « to bench » (`docs/MODELS.md:78-79`). **No French measurement exists in the project.**
- Lexical half: an OR over every word of four letters or more of the whole sentence (`index_store.rs` around 1240), one flat bonus of +0.2 for the best few hits (`retrieval.rs` around 220). No stemming (« courriers » ≠ « courrier »), no field weights, no phrase or proximity, no identifier normalisation (an AVS number is split on its dots).
- No fusion of rankings, no reranker (`providers/ollama.py:113` refuses `rerank`), no metadata fields, no structure, chunks of 1 200 characters cut page by page (`chunking.rs:12`).
- Every question scores every chunk by brute force, text included (`index_store.rs:895`).

## 3. The catalogue

Verdict legend: **ADOPT** (build it) · **MEASURE** (build it behind a switch and decide on figures) · **LATER** · **REJECT**.

### A. At indexing time (the Analyse is ours: hand-started, visible, cancellable)

| # | Method | What it gives | Cost and fit | Verdict |
| --- | --- | --- | --- | --- |
| A1 | **Structure-aware chunking**: split on headings, paragraphs, sentences; tables and lists never cut, header row repeated. The chunk keeps its section path (« 3.2 Conditions > Franchise »). | The strongest, cheapest gain on long and structured documents. A 2025 peer-reviewed study found chunking configuration has **as much influence as the embedding model** ([Vectara, arXiv 2410.13070 **(primary)**](https://vectara.com/blog/is-semantic-chunking-worth-the-computational-cost)). | Deterministic, free. Already planned in session 2. | **ADOPT** |
| A2 | **Chunk size**: smaller, around 200-400 tokens with 10-20 % overlap, for factual lookups. Practitioner guides recommend 256-512 tokens for factoid queries, 512-1 024 for analytical ones; recursive splitting at 400-512 tokens is the most repeated default ([2026 benchmark guide](https://blog.premai.io/rag-chunking-strategies-the-2026-benchmark-guide), *unverified numbers*). | Higher precision per passage, less text sent (the tower reads about 90 tokens a second). Cost: more passages to embed (about 0.74 s each). | Make it a **measured parameter**: test 800 / 1 200 / 1 600 characters on the exam. | **MEASURE** |
| A3 | **Contextual prefix, deterministic**: prepend to the *embedded and indexed* text (not to the cited text) the document title, type, author, date and section path. | The cheap form of Anthropic's contextual retrieval. | Free, deterministic, a few lines per chunk. Needs A1 and A5. | **ADOPT** |
| A4 | **Contextual prefix written by a model** (Anthropic's « contextual retrieval »): one model call per chunk writes 50-100 tokens of context, prepended before embedding and BM25. Reported on top-20 retrieval failure: 5.7 % → 3.7 % (contextual embeddings), → 2.9 % (+ contextual BM25), → 1.9 % (+ reranking) ([summary](https://www.maginative.com/article/anthropics-contextual-retrieval-technique-enhances-rag-accuracy-by-67), [analysis](https://gradientflow.com/contextual-retrieval-rag/); figures are from the best embedding model, gains vary by domain from 41 % to 84 %). | Real gain on chunks that say « the company » or « he » without naming it: letters, results, minutes. | **Heavy**: a model call per chunk (4 000 for a 1 000-page book). Acceptable only inside a hand-started Analyse, with a duration estimate, ideally on the Mac mini. | **LATER**, after A3 is measured |
| A5 | **Metadata fields** (section 5): title, author, dates, document type, sender, recipient, subject, reference numbers, language. Indexed as separate full-text columns and usable as filters. | « Quels courriers viennent du Dr X », « en mars 2026 », « les factures » become exact lookups. | Deterministic extraction, free. | **ADOPT** |
| A6 | **Text cleaning**: strip headers and footers repeated on every page, repair hyphenation across lines and pages, normalise ligatures, Unicode form, spaces inside numbers, drop empty or boilerplate chunks, mark low-confidence OCR chunks. | Removes noise that pollutes both rankings; fewer filler passages. | Deterministic. | **ADOPT** |
| A7 | **Document expansion with generated questions** (doc2query): a model writes the questions each passage answers; they are indexed with the passage. Original paper: BM25 improved about 15 % on two collections, recall@1000 85.3 → 89.3 ([Nogueira et al., arXiv 1904.08375 **(primary)**](https://arxiv.org/pdf/1904.08375)); generated queries add off-topic terms, hence the filtered variant doc2query-- ([SIGIR 2022](https://cs.uwaterloo.ca/~jimmylin/publications/Ma_etal_SIGIR2022.pdf)). | Bridges vocabulary gaps (« franchise » vs « participation aux coûts »). | A model call per chunk, like A4. | **LATER** |
| A8 | **Keyphrases per chunk** (RAKE, YAKE, TextRank) and the knowledge base's entities attached per passage (`kb_mentions.chunk_id` already exists). | Cheap extra lexical field; per-passage targeting. | Deterministic. | **ADOPT** (entities); **MEASURE** (keyphrases) |
| A9 | **Document-level digest** (title, outline, first sentence of each section): a short « document card » chunk per file, indexed and embedded. | A question about a book or a letter as a whole finds its card, not 6 random pages. | Deterministic (session 2, lot 8, stage T0). | **ADOPT** |
| A10 | **Semantic chunking** (split where the embedding similarity drops). | In the Vectara study, fixed-size chunking **beat** semantic chunking on realistic documents; semantic fragments are very short and the compute is high. | Costly, inconsistent. | **REJECT** |
| A11 | **Late chunking** (embed the whole document with a long-context model, then pool per chunk). About +2-4 % relative nDCG in Jina's own tests ([Jina](https://jina.ai/news/late-chunking-in-long-context-embedding-models/)). | Small gain, needs token-level embeddings from a long-context model: **not available through our embedding route** (the gateway returns one vector per text). | | **REJECT** for now |
| A12 | **RAPTOR / GraphRAG** (trees or graphs of LLM summaries). | Aimed at multi-document synthesis, very heavy to build and refresh. | | **REJECT** (the digest of A9 and the entity base cover the need) |

### B. The embedding model

| # | Model (licence per the project's own policy, `docs/MODELS.md`) | Notes | Verdict |
| --- | --- | --- | --- |
| B1 | `nomic-embed-text` (v1.5, Apache 2.0) | Today's. 768 dimensions, 8 192-token context, small and fast. English-oriented per third-party sources. **Not measured on French.** | Baseline |
| B2 | **`bge-m3`** (MIT, 568M parameters, 1 024 dimensions, 8 192-token context, 100+ languages). It is on her disk already (`models/LICENSES.md:11`). Strong open multilingual workhorse ([guide](https://openrouter.ai/blog/insights/best-embedding-models-2026/)). Native dense, sparse and multi-vector outputs, but **Ollama returns dense vectors only** *(unverified)*. | About twice the size of nomic, so a slower Analyse. | **MEASURE first** |
| B3 | **`qwen3-embedding`** 0.6B / 4B / 8B (Apache 2.0, 100+ languages, 32K context; the 8B led the multilingual MTEB leaderboard in mid-2025 with 70.58) ([Ollama guide](https://www.glukhov.org/rag/embeddings/qwen3-embedding-qwen3-reranker-on-ollama/), [ranking](https://www.premai.io/blog/best-embedding-models-for-rag-2026-ranked-by-mteb-score-cost-and-self-hosting/)). 0.6B is cheap; 4B-8B belong to the Mac mini. | | **MEASURE** (0.6B now, 4B on the Mac mini) |
| B4 | `snowflake-arctic-embed2`, `granite-embedding`, `nomic-embed-text-v2-moe` (the project lists the first two as Apache 2.0, `docs/MODELS.md:79`). The v2-moe is multilingual but limited to 512 tokens of input. | Not searched in depth. | **MEASURE** if B2/B3 disappoint |
| B5 | Excluded by the licence policy: Jina v2/v3 rerankers and embeddings (CC-BY-NC), models under custom terms. | | **REJECT** |

No public French leaderboard of 2026 was found; the French benchmark exists ([MTEB-French, arXiv 2405.20468 **(primary)**](https://arxiv.org/html/2405.20468)) but standings must be checked live. **The decision is made on her own exam (section 6), not on a leaderboard.**

### C. The lexical half (SQLite FTS5 is already there and good enough to improve, not replace)

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| C1 | **French analysis at index and query time**: `unicode61 remove_diacritics` (already the default: « Genève » = « Geneve »), elision (`l'`, `d'`, `qu'`), and a **Snowball French stemmer** so « courriers / courrier », « factures / facture ». FTS5 ships only an English `porter` stemmer, and a custom tokenizer needs the C API. A path without FFI: **stem in Rust and store the stemmed tokens in an extra FTS column**, stemming the query the same way (the `rust-stemmers` crate has a French algorithm *(unverified, check licence and version)*). | Raises recall on inflected words for free. | **ADOPT**, behind a switch |
| C2 | **Field weights**: FTS5's `bm25()` takes one weight per column, so title, section path, author and subject can count more than body text ([FTS5](https://sqlite.org/fts5.html)). | Needs A5 columns. | **ADOPT** |
| C3 | **Phrase and proximity**: quoted phrases (`"franchise annuelle"`), `NEAR(a b, 10)` so two query words must be close. | Precision for multi-word terms and names. | **ADOPT** |
| C4 | **Trigram tokenizer** for typos, partial words and codes (needs SQLite 3.34 or later; the project bundles SQLite through `rusqlite` with the `bundled` feature) ([note on versions](https://github.com/groue/GRDB.swift/pull/1655)). | A second FTS table; recall on misspelled names and partial identifiers. | **MEASURE** |
| C5 | **Identifier normalisation**: AVS numbers, social-security numbers, IBANs, invoice numbers stored with and without separators as one token. | The knowledge base already hashes identifiers; the *text* search does not match `756.9617.2765.34` as one token. | **ADOPT** |
| C6 | **Synonyms and domain thesaurus** (data files per knowledge pack, never in code): LAMal ↔ assurance-maladie, CPAM ↔ caisse primaire, franchise ↔ participation. Applied at query time as OR-groups. | Deterministic, owned, per country pack (`health-fr`, `health-ch`). | **ADOPT** (small seed list, grown from misses) |
| C7 | **Tantivy** (Rust, Lucene-like: BM25, phrase, prefix, field boosts, French stemmers) in place of FTS5 ([Turso's rationale](https://turso.tech/blog/beyond-fts5)). | A second index to keep consistent with SQLite and a bigger build. FTS5 + C1-C5 covers the need. | **REJECT** for now |

### D. Fusing the rankings

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| D1 | **Reciprocal Rank Fusion** of the lexical list, the vector list and the entity list: score = Σ 1/(60 + rank). The standard recipe for SQLite FTS5 + vectors ([Willison](https://simonwillison.net/2024/Oct/4/hybrid-full-text-search-and-vector-search-with-sqlite/), [scrydb, arXiv 2608.24060](https://arxiv.org/pdf/2608.24060)). | Replaces the flat +0.2 bonus, robust because it ignores the scales of the scores. | **ADOPT** |
| D2 | **Weighted fusion** (alpha between dense and lexical) tuned on the exam. | RRF with per-list weights. | **MEASURE** |
| D3 | **Candidate generation first**: take the union of the top candidates of every channel (lexical, entity, metadata filters, vector), then score vectors **only on that union** when the scope is large. | Removes the full scan (`all_chunks`) and keeps recall. Session 2, lot 5. | **ADOPT** |

### E. Reranking (precision at the top)

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| E1 | **Cross-encoder reranker** on the top 30-50 candidates. Anthropic's figures: reranking took failure 2.9 % → 1.9 % ([summary](https://www.maginative.com/article/anthropics-contextual-retrieval-technique-enhances-rag-accuracy-by-67)). `bge-reranker-v2-m3` is Apache 2.0, multilingual, about 568M parameters, workable on CPU for modest volumes; Jina's v2 multilingual reranker is CC-BY-NC and excluded ([comparison](https://futureagi.com/blog/best-rerankers-for-rag-2026/)). `Qwen3-Reranker` (0.6B-8B) also exists on Ollama as a model that a client scores by prompting, since **Ollama has no rerank endpoint** ([Go tutorial](https://glukhov.org/fr/rag/reranking/reranking-with-ollama-qwen3-reranker-golang/); our own `providers/ollama.py:113` refuses `rerank`). | **Not measured here**: no CPU latency figure was found. Two ways in: an ONNX runtime on the workstation (`fastembed-rs`, Apache 2.0, `TextRerank`, [docs](https://docs.rs/fastembed/=6.0.3/); Windows/macOS packaging to verify) or a rerank service on the AI machine behind the existing `AIProvider.rerank` port. | **MEASURE** (likely the biggest precision gain after hybrid) |
| E2 | **MMR / de-duplication**: do not return five near-identical passages; one per section unless the section is the answer. | Deterministic. | **ADOPT** |

### F. Understanding the question without a model

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| F1 | **Strip the question frame** (« Quels documents parlent de ... ») to keep the content terms. Session 1. | Fixes the dilution of the lexical half. | **ADOPT** |
| F2 | **Entities** (the knowledge base) and **identifiers** extracted from the question, as today, plus the **roles** (« courriers *du* Dr Marc Bersier » = author). | Already built; extend with roles. | **ADOPT** |
| F3 | **Dates and document types as filters**: « en mars 2026 », « les factures », « la dernière lettre ». The table engine already parses date phrases; reuse its parser on the new date metadata. | | **ADOPT** |
| F4 | Query expansion by the model, HyDE (a hypothetical answer is embedded), multi-query. | Costs a model call *before* retrieval and adds the model's errors to the search; against the owner's aim. | **REJECT** as a default |
| F5 | Pseudo-relevance feedback (RM3: expand with terms of the top results). | Deterministic, a second pass; gain uncertain in French. | **MEASURE** later |

### G. Assembling what the model reads

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| G1 | **Small-to-big (parent-document)**: search small passages, send the passage with its neighbours or its section header; it degrades gracefully ([overview](https://futureagi.com/blogs/advanced-chunking-techniques-for-rag)). Bounded by the character cap. | Session 2, lot 6. | **ADOPT** |
| G2 | **Cap per section / page**, not per file. | Session 2. | **ADOPT** |
| G3 | **Order**: put the best passages at the start and the end of the context; the « lost in the middle » effect shows with many passages and is inconsistent across studies ([arXiv 2410.05983](https://arxiv.org/pdf/2410.05983), [2026 reproducibility study](https://www.emergentmind.com/papers/2605.27105)). With few passages it does not matter. | Free. | **ADOPT** |
| G4 | **Fewer passages**: the tower pays about 90 tokens a second. 4-6 excellent passages beat 30 mediocre ones. | The aim of both sessions. | **ADOPT** |

### H. Scale of the vector store

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| H1 | **int8 quantisation** of the stored vectors: negligible loss (1-3 %), a quarter of the memory; **binary + rescoring** keeps about 96 % at 32× less memory ([Sentence-Transformers](https://sbert.net/examples/applications/embedding-quantization/README.html)). | | **MEASURE** (int8) |
| H2 | **Matryoshka truncation** (only for models trained for it; nomic v1.5 and Qwen3 are): 1 024 → 512 dimensions costs about 1.4 % ([report](https://arxiv.org/pdf/2601.04720)). | | **MEASURE** |
| H3 | **ANN index** (HNSW; `sqlite-vec`, `usearch`). | Needed only beyond roughly 100 000 passages; a candidate-first scan (D3) is enough before. Cross-platform builds to verify. | **LATER** |
| H4 | **ColBERT / multi-vector (late interaction)**. | Heavy storage and a runtime we do not have. | **REJECT** for now |

### I. The model's side: do not count on it, but constrain it

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| I1 | **Temperature 0** (the desktop sends none today; the server accepts it). | Session 1, lot A. | **ADOPT** |
| I2 | **Passage ids**: number the passages, ask the model to cite ids (`[3]`), and let the application map ids to file, section and page. | The model never writes a path (it garbles them). The application already numbers passages (`format_evidence`). | **ADOPT** |
| I3 | **Structured output**: Ollama's `format` parameter takes a JSON schema and constrains decoding ([Ollama](https://registry.ollama.ai/blog/structured-outputs)); small models still need parse-and-repair. Useful to force `{"claims":[{"text":..., "passage_ids":[...]}]}`. | Test on `ministral-3:3b`; keep a plain-text fallback. | **MEASURE** |
| I4 | **Verification after the answer**: figures (exists), quotations must be substrings of the passages (session 2, lot 9), ids must exist. Anomalies appended, never rewriting the answer. | | **ADOPT** |
| I5 | **Extractive first**: show « Passages trouvés » (highlighted terms, section, page, author, date, « Ouvrir à la page ») **before and independently of** the answer. | This is what makes the system usable when the model fails. | **ADOPT** |

### J. Measuring, so that nothing is believed

| # | Method | Notes | Verdict |
| --- | --- | --- | --- |
| J1 | **Passage-level ground truth**: today `questions.json` gives the files that hold each answer; add the exact page and section (planted facts, session 2 lot 0). | Without it, chunk quality cannot be measured. | **ADOPT** |
| J2 | **Metrics**: recall@k (k = 5, 10, 30), MRR, nDCG@10, plus « files wrong » and « time ». | Same exam, every change. | **ADOPT** |
| J3 | **Ablation switches**: each method above behind a switch so its contribution is measured alone. | | **ADOPT** |

## 4. What I would not do

Fine-tuning an embedding model; semantic chunking; a graph of summaries; query rewriting by the model; any step that needs the model to be right for the right passage to be found.

## 5. Author and metadata: what can be read deterministically

| Source | Fields | Notes |
| --- | --- | --- |
| **PDF** | Info dictionary and XMP: Title, Author, Subject, Keywords, Creator, Producer, CreationDate, ModDate; the outline (bookmarks); the number of pages | `pdfium-render` is already a dependency. Many scans and exports carry no author or a software name: treat as weak. |
| **DOCX** | `docProps/core.xml`: title, creator, last modified by, created, modified, keywords, subject; `docProps/app.xml`: pages, words; heading styles; tracked-change and comment authors | `zip` and `quick-xml` already used (`extraction.rs:277`). |
| **TXT / MD** | Front matter (title, author, date), first lines | |
| **Mail files** (`.eml`, later) | From, To, Date, Subject | Out of scope until the format is read (`docs/ROADMAP`). |
| **Letter and form text** (the real « author » of a letter) | Sender block at the top (« Dr Jean Dupont, Hôpital ... »), recipient, place and date (« Genève, le 4 août 2026 »), « Objet : », reference numbers, signature line | Pattern packs per country and language, as data, in the knowledge packs; the entity base already knows doctors, titles and organisations, so the **author is an entity with the role `author`**, not a free string. This is the field that answers « quels courriers viennent du Dr X ». |
| **File name and path** | Date prefix, document type words, folder (`courriers-entrants/2026-08`) | Already clean by the file-name rule. |
| **File system** | Modified time | Weakest; last resort for « le plus récent ». |

Proposed storage: one row per document in a `document_fields` table (type, title, authors with roles, recipients, dates with their kind, subject, references, language, source of each field and a confidence), filled during the Analyse; the text fields mirrored into FTS5 columns for C2 weights; dates and types used as filters (F3). Each field keeps **where it came from** (PDF property, header block, file name) so the interface can show it and the owner can trust or correct it.

## 6. How to proceed: measure before building

The Vectara result (chunking matters as much as the embedder) and the lack of any French measurement in the project say the same thing: **do not choose from blogs.** Proposed first step, a **retrieval lab** inside session 1's lot 0 (or a session of its own), reusing the scoring tool:
1. Passage-level truth for the 205 questions and for the planted-fact long documents.
2. Baseline of the current pipeline (files and passages, recall@5/10/30, MRR).
3. One switch at a time, in this order: question frame (F1) → RRF replacing the flat bonus (D1) → French stemming and field columns (C1, C2) → deterministic prefix (A3) → chunk size (A2) → embedding model (B2, B3) → cross-encoder (E1) → int8 (H1).
4. Keep a method only if recall@10 rises and nothing regresses; record the figures and the time cost of the Analyse (the embedding model changes it).
Note the cost: a larger embedding model slows every Analyse (nomic: about 0.74 s a passage on the tower); `bge-m3` is about twice the size. The Mac mini changes that arithmetic, so the lab runs again there.

## 7. Where each item lands

- **Session 1 (retrieval v2)**: F1, F2 roles, F3, D1, C1-C3, C5, C6, E2, G3, I1, I2, I5, J1-J3, plus the lab. Mostly query-side and lexical; no ingestion change. The metadata filters need A5, so only the filename- and inventory-based ones land here.
- **Session 2 (long documents)**: A1, A3, A5, A6, A8, A9, G1, G2, D3, H1, I4, the structure and the resumable ingestion.
- **A third, later**: A4 and A7 (model-written context and questions, on the Mac mini), E1 if its latency is acceptable, H3, the facts table.

## 8. Decisions for the owner

1. Run the retrieval lab first (my recommendation) or build the sessions as planned and measure inside them.
2. Accept a possible change of the embedding model (a full re-Analyse, slower) if the lab shows a clear gain on French.
3. A reranker on the workstation (ONNX) or on the AI machine, if the lab shows its gain.
4. « Passages trouvés » as a first-class display, ahead of the answer.
5. The author as an entity with a role, fed by header-block patterns per country pack.

## 9. Sources

Primary: [Vectara, chunking study (arXiv 2410.13070)](https://vectara.com/blog/is-semantic-chunking-worth-the-computational-cost) · [doc2query (arXiv 1904.08375)](https://arxiv.org/pdf/1904.08375) · [MTEB-French (arXiv 2405.20468)](https://arxiv.org/html/2405.20468) · [mGTE (arXiv 2407.19669)](https://arxiv.org/pdf/2407.19669) · [Retrieval reordering (arXiv 2410.05983)](https://arxiv.org/pdf/2410.05983) · [Docling (arXiv 2408.09869)](https://arxiv.org/pdf/2408.09869v4) · [SQLite FTS5](https://sqlite.org/fts5.html).
Secondary: [Contextual retrieval summary](https://www.maginative.com/article/anthropics-contextual-retrieval-technique-enhances-rag-accuracy-by-67) · [Gradient Flow](https://gradientflow.com/contextual-retrieval-rag/) · [Embedding models 2026](https://openrouter.ai/blog/insights/best-embedding-models-2026/) · [Embedding ranking](https://www.premai.io/blog/best-embedding-models-for-rag-2026-ranked-by-mteb-score-cost-and-self-hosting/) · [Hybrid search in SQLite](https://simonwillison.net/2024/Oct/4/hybrid-full-text-search-and-vector-search-with-sqlite/) · [scrydb](https://arxiv.org/pdf/2608.24060) · [Rerankers 2026](https://futureagi.com/blog/best-rerankers-for-rag-2026/) · [Qwen3 embedding and reranker on Ollama](https://www.glukhov.org/rag/embeddings/qwen3-embedding-qwen3-reranker-on-ollama/) · [fastembed-rs](https://docs.rs/fastembed/=6.0.3/) · [Late chunking](https://jina.ai/news/late-chunking-in-long-context-embedding-models/) · [Chunking guide 2026](https://blog.premai.io/rag-chunking-strategies-the-2026-benchmark-guide) · [Embedding quantization](https://sbert.net/examples/applications/embedding-quantization/README.html) · [Ollama structured outputs](https://registry.ollama.ai/blog/structured-outputs) · [Tantivy in Turso](https://turso.tech/blog/beyond-fts5).
