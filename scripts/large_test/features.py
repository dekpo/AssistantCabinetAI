"""The features of the application as it stands on the restart state of 16 October 2026 (commit `fe15e03` and the
readers built on it), each with the way a question about it is judged.

A feature of the set-aside branches (passage blocks, summaries, dates and amounts as facts, the refusal check, the
reranker) is not here: it does not exist in this build. The table `docs/test-reports/large-test/FEATURES.md` is
generated from this list and from the book, so a feature without a question is visible.

`judged` says what the scorer can decide by itself:

    facts     words, a number or a date that must appear in the answer
    sources   the files (and pages) the answer cites must include the files that hold it
    route     the application must take the road the feature is about
    listing   the complete list of files, or the count, of a deterministic answer
    refusal   the product must say it does not know, and must not invent
    columns   the columns, sheets or rows a table answer names
    nudge     the engine must refuse for the reason the feature is about
    human     a person reads the answer (said so in the book: a short sheet, never all 300)
"""

from __future__ import annotations

# id -> (block, name, what it is, judged by)
FEATURES = {
    # --- documents ---------------------------------------------------------------------------------------------
    "D01": ("documents", "fact_from_document", "A fact asked about one document, answered with its sources (file and page).", "facts, sources, route"),
    "D02": ("documents", "lookalike_names", "People with the same surname or the same given name: what is said of one is never said of the other.", "facts, must-not, sources"),
    "D03": ("documents", "identifiers_in_text", "An invoice, contract, social-security, health-insurance or IBAN number found in the text and used to find the file.", "facts, sources"),
    "D04": ("documents", "organisations", "An organisation by its full name or its acronym; an organisation that carries a surname.", "sources, human"),
    "D05": ("documents", "set_search", "« Quels documents parlent de X ? »: the complete list and the count, from the index, no model and no embedding.", "listing, route"),
    "D06": ("documents", "set_search_identifier", "The same, for an identifier typed in another spelling than the one written in the files.", "listing, route"),
    "D07": ("documents", "rank_by_date", "« Quel est le dernier courrier reçu ? »: ranked by the date of the file, saying how.", "listing, route"),
    "D08": ("documents", "folder_questions", "Questions about the folder itself (how many documents, how many unreadable, which formats), answered with no model.", "listing, route"),
    "D09": ("documents", "named_file", "A question that names a file reads that file and no other.", "facts, sources, route"),
    "D10": ("documents", "follow_up", "A short question read together with the previous one.", "facts, sources"),
    "D11": ("documents", "absent_information", "The documents do not carry the answer: the product says so instead of generating one.", "refusal"),
    "D12": ("documents", "isolation_selection", "A file that is not ticked is never read; its content never reaches the answer.", "must-not, sources"),
    "D13": ("documents", "small_selection", "A handful of ticked files: the answer comes from them.", "facts, sources"),
    "D14": ("documents", "long_document", "A fact deep in a document of 100 pages or more, located by page and section.", "facts, sources, page"),
    "D15": ("documents", "scanned_document", "A scanned page read by the local text recognition.", "facts, sources, human"),
    "D16": ("documents", "reader_vtt", "A WebVTT transcript of several hundred cues.", "facts, sources"),
    "D17": ("documents", "reader_srt", "A SubRip subtitle file of several hundred cues.", "facts, sources"),
    "D18": ("documents", "reader_rtf", "A Rich Text Format document.", "facts, sources"),
    "D19": ("documents", "reader_eml", "An e-mail message saved as a file.", "facts, sources"),
    "D20": ("documents", "reader_odt", "An OpenDocument text.", "facts, sources"),
    "D21": ("documents", "reader_odp", "An OpenDocument presentation, a slide being a page.", "facts, sources, page"),
    "D22": ("documents", "reader_pptx", "A PowerPoint presentation, a slide being a page, in the order of the deck.", "facts, sources, page"),
    "D23": ("documents", "amounts", "An amount asked: the figure must be the one written, and a figure written nowhere is flagged.", "facts"),
    # --- spreadsheets ------------------------------------------------------------------------------------------
    "T01": ("spreadsheets", "structure", "Sheets, columns and rows of a workbook, from its inventory.", "columns, route"),
    "T02": ("spreadsheets", "aggregates", "Sum, mean, median, minimum, maximum, computed by the engine.", "facts, route"),
    "T03": ("spreadsheets", "filter_value", "A count or a total restricted to a value written in a column.", "facts, route"),
    "T04": ("spreadsheets", "filter_date", "A weekday, a month, a year or a range of dates.", "facts, route"),
    "T05": ("spreadsheets", "filter_number", "A comparison with a number (greater than, less than).", "facts, route"),
    "T06": ("spreadsheets", "groups", "The largest group, the total per group, the first N groups.", "facts, route"),
    "T07": ("spreadsheets", "distinct_values", "The different values of a column.", "facts, route"),
    "T08": ("spreadsheets", "trap_title_row", "The header is on row 3, under a title.", "facts, route"),
    "T09": ("spreadsheets", "trap_merged_title", "The title is a merged cell over the table.", "facts, route"),
    "T10": ("spreadsheets", "trap_formula", "A column that ends with a SUM formula: the engine refuses to total it, and says why.", "nudge, route"),
    "T11": ("spreadsheets", "trap_empty_column", "A column with nothing in it, in the middle of the table.", "facts, route"),
    "T12": ("spreadsheets", "trap_notes_sheet", "A second sheet of free text: the engine asks which sheet, never guesses.", "nudge, route"),
    "T13": ("spreadsheets", "trap_text_dates", "Dates written as text (dd/mm/yyyy, dd.mm.yyyy).", "facts, route"),
    "T14": ("spreadsheets", "trap_locale_numbers", "Numbers written 1 234,56 or 1'234.56.", "facts, route"),
    "T15": ("spreadsheets", "format_xlsx", "An Excel workbook.", "facts, route"),
    "T16": ("spreadsheets", "format_csv", "A CSV export, in UTF-8 or in Windows-1252.", "facts, route"),
    "T17": ("spreadsheets", "format_ods", "An OpenDocument spreadsheet.", "facts, route"),
    "T18": ("spreadsheets", "which_workbook", "Several workbooks are ticked and the question names none: the product asks, never picks.", "route"),
    "T19": ("spreadsheets", "unknown_or_unselected_file", "A workbook that does not exist, or that is not ticked.", "route"),
    "T20": ("spreadsheets", "honest_refusals", "A value found in no column, or in two: the product says so and offers what exists.", "nudge, route"),
    "T21": ("spreadsheets", "model_plan", "A question the classifier cannot read: the model proposes a plan, the engine computes it.", "facts, route"),
    # --- mixed -------------------------------------------------------------------------------------------------
    "M01": ("mixed", "entity_link", "A person named in a document and found in the rows of a workbook: both sides answer.", "facts, sources, route"),
    "M02": ("mixed", "document_against_table", "An amount in a document compared with the same line of a workbook.", "facts, route"),
    "M03": ("mixed", "routing", "With both kinds ticked, a question about one side is answered by that side.", "route, facts"),
    "M04": ("mixed", "mixed_folder", "« Combien de documents ai-je ? » with tables ticked: documents counted, not rows.", "listing, route"),
    "M05": ("mixed", "mixed_set", "« Quels documents parlent de X ? » covers the tables too: one list per folder.", "listing, route"),
    "M06": ("mixed", "fill_plan", "A request to write a letter for each row: a plan to approve, nothing written.", "route"),
}

# Features of the interface that are not a question: listed in the report as checked by hand in the script.
BY_HAND = {
    "Analyser": "The Analyse button, its progress and its summary lines (counted in the report from analyse.json).",
    "Arrêter": "The stop button of an Analyse and of an answer.",
    "Recherche par sens": "The vector share « vectorisé à x % » on the card.",
    "Effacer la conversation": "The button that clears the conversation.",
    "Documents utilisés": "The selection list: ticking files, « tous », « aucun ».",
    "Champ de recherche": "The search field of the folder cards (the finder) and its filters.",
    "@": "The « @ » picker, the name chips and « Élargir à ma sélection » (needs the entities of the live index).",
    "Suggestions": "The suggested questions.",
    "Gérer…": "The knowledge management dialog.",
}


def block_of(feature: str) -> str:
    return FEATURES[feature][0]
