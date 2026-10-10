# Human test - lot 2 bis, review of name pairs

Written for the owner. English; every name is quoted as data. Branch `feat/kb-names-calibration`. No application to start, no service, **15 to 20 minutes**, 88 rows.

## What this is

The two phonetic keys (French `fr-rules`, English `en-rules`) were checked against the complete public lists of surnames and first names (Insee for French, US Census and SSA for English). Everything that a machine can decide has been decided: no empty key, no key that swallows dozens of names, no masculine and feminine first name sharing a key unless they are the same sound in the language. What a machine cannot decide is below: whether two names that the keys treat in a given way really are the same name. A pre-filled answer is on every row. **You mark only the rows where you disagree.**

Every name below is one of the most frequent in a public list of aggregates. None is a person's record.

## How to answer

1. Read the proposal on a row. If you agree, do nothing.
2. If you disagree, write `x` in the last column of the file, or simply reply in the chat with the ids of the rows (for example `FS9, FF3, M7`). One letter, never a name to type, never a JSON file to edit.
3. A name you do not know (an English first name, say): leave the row, I keep my proposal.

## If you are short of time

The rows are in order of usefulness. Tables 1 and 2 (French) are the pilot's language and decide the length rule below: do those first (24 rows, about 6 minutes). Then tables 3 and 4 (English, 24 rows), then the misses (20 rows, they decide whether a rule changes), the hits last (20 rows). You can stop after any table.

## What I found before asking you

| List | Pairs of names that share a key and differ by one letter (4 letters or more) | Same key, spelled further apart | Different keys, differ by a usual spelling variation |
| --- | --- | --- | --- |
| French surnames (Insee) | 476 | 472 | 123 |
| French first names (Insee) | 1 144 | 872 | 249 |
| English surnames (US Census) | 275 | 157 | 64 |
| English first names (US SSA) | 1 160 | 948 | 139 |

That is 5 000-plus candidate pairs in all, far above the cap. What I dropped: everything but the pairs of the 5 000 most frequent names of each list, then I kept, by combined frequency, 4 pairs at 4 letters, 4 at 5 letters and 4 at 6 letters or more per list (never the same name twice), 5 hits per list and one miss per kind of spelling edit, 5 per list. That is 48 + 20 + 20 = 88 rows.

## The three questions

- **Tables 1 to 4, "did you mean?"** Somebody types or dictates A, and B is the only one of the two in the selection. Is it right for the product to ask "did you mean B?" Proposal `yes` for two spellings or two homophones of one name, `no` for two names that are clearly two people or two different names.
- **Table 5, hits.** The keys say A and B sound the same although they are spelled further apart. Do they sound the same to you? Proposal `yes` or `no`.
- **Table 6, misses.** The keys say A and B do not sound the same although they differ by a letter that spelling variants usually change. Do they in fact sound the same? A proposal `no` that you mark `x` means a rule should change.

## 1. French surnames

| id | pair | frequencies | key | length | proposal | your mark |
| --- | --- | --- | --- | --- | --- | --- |
| FS1 | Roux / Roue | 75365 / 3460 | `RU` | 4 | yes | |
| FS2 | Joly / Jolly | 45336 / 11803 | `JOLI` | 4 | yes | |
| FS3 | Marie / Mari | 48635 / 2525 | `MARI` | 4 | yes | |
| FS4 | Roche / Roch | 42580 / 6097 | `ROX` | 4 | yes | |
| FS5 | Thomas / Tomas | 118331 / 2424 | `TOMA` | 5 | yes | |
| FS6 | Durand / Duran | 108374 / 2859 | `DYRa` | 5 | yes | |
| FS7 | Leroy / Leroi | 87282 / 2434 | `LERUA` | 5 | yes | |
| FS8 | Bonnet / Bonet | 68481 / 1855 | `BONE` | 5 | yes | |
| FS9 | Martin / Martins | 250013 / 14172 | `MARTe` | 6 | no (two surnames) | |
| FS10 | Lefebvre / Lefevre | 91459 / 64107 | `LEFEVR` | 8 | yes | |
| FS11 | Bernard / Bernhard | 131330 / 2581 | `BERNAR` | 7 | yes | |
| FS12 | Gauthier / Gautier | 57731 / 53164 | `GOTIE` | 8 | yes | |

## 2. French first names

| id | pair | frequencies | key | length | proposal | your mark |
| --- | --- | --- | --- | --- | --- | --- |
| FF1 | René / Reine | 516650 / 34180 | `REN` | 4 | no (man / woman) | |
| FF2 | Éric / Erick | 321225 / 6910 | `ERIK` | 4 | yes | |
| FF3 | Marc / Mark | 238475 / 2245 | `MARK` | 4 | yes | |
| FF4 | Annie / Anie | 210200 / 665 | `ANI` | 4 | yes | |
| FF5 | Alain / Allain | 507010 / 2965 | `ALe` | 5 | yes | |
| FF6 | Henri / Henry | 407190 / 12125 | `aRI` | 5 | yes | |
| FF7 | David / Davide | 316050 / 580 | `DAVID` | 5 | no (Italian form) | |
| FF8 | Thomas / Tomas | 282110 / 2100 | `TOMA` | 5 | yes | |
| FF9 | Georges / Georget | 406500 / 1340 | `JEORJE` | 7 | no | |
| FF10 | Nicolas / Nicola | 406400 / 1150 | `NIKOLA` | 6 | no (Italian form) | |
| FF11 | Cathérine / Katherine | 394920 / 1535 | `KATERIN` | 8 | yes | |
| FF12 | Patrick / Patric | 395245 / 965 | `PATRIK` | 7 | yes | |

## 3. English surnames

| id | pair | frequencies | key | length | proposal | your mark |
| --- | --- | --- | --- | --- | --- | --- |
| ES1 | Diaz / Dias | 347636 / 16044 | `DIAS` | 4 | yes | |
| ES2 | Cook / Cooke | 302589 / 33223 | `KUK` | 4 | yes | |
| ES3 | Reed / Read | 277030 / 17048 | `ReD` | 4 | yes | |
| ES4 | Chen / Shen | 169580 / 12839 | `XEN` | 4 | no (two surnames) | |
| ES5 | Smith / Smyth | 2442977 / 9470 | `SMIT` | 5 | yes | |
| ES6 | Brown / Browne | 1437026 / 22289 | `BRoN` | 5 | yes | |
| ES7 | Davis / Davies | 1116357 / 33753 | `DAVIS` | 5 | yes | |
| ES8 | Lopez / Lopes | 874523 / 18310 | `LOPES` | 5 | yes | |
| ES9 | Rodriguez / Rodrigues | 1094924 / 31280 | `RODRIGUS` | 9 | yes | |
| ES10 | Martinez / Martines | 1060159 / 9363 | `MARTINES` | 8 | yes | |
| ES11 | Gonzalez / Gonzales | 841025 / 214758 | `GONSALES` | 8 | yes | |
| ES12 | Hernandez / Hernandes | 1043281 / 8612 | `ERNANDES` | 9 | yes | |

## 4. English first names

| id | pair | frequencies | key | length | proposal | your mark |
| --- | --- | --- | --- | --- | --- | --- |
| EF1 | Mary / Mari | 4156654 / 16645 | `MARI` | 4 | yes | |
| EF2 | Sarah / Sara | 1101838 / 437441 | `SARA` | 4 | yes | |
| EF3 | Mark / Marc | 1366672 / 141137 | `MARK` | 4 | yes | |
| EF4 | Eric / Erik | 893184 / 158126 | `ERIK` | 4 | yes | |
| EF5 | Thomas / Tomas | 2367087 / 30730 | `TOMAS` | 5 | yes | |
| EF6 | Brian / Bryan | 1178298 / 391306 | `BRIAN` | 5 | yes | |
| EF7 | Linda / Lynda | 1458899 / 78098 | `LINDA` | 5 | yes | |
| EF8 | Susan / Suzan | 1126022 / 9081 | `SUSAN` | 5 | yes | |
| EF9 | Christopher / Cristopher | 2079103 / 11898 | `KRISTOFER` | 11 | yes | |
| EF10 | Daniel / Daniele | 1991091 / 4344 | `DANIL` | 6 | no (man / woman) | |
| EF11 | Elizabeth / Elisabeth | 1693973 / 48045 | `ELISABET` | 9 | yes | |
| EF12 | Matthew / Mathew | 1659655 / 77745 | `MATU` | 7 | yes | |

## 5. Hits - same key, spelled further apart (do they sound the same?)

| id | pair | frequencies | key | list | proposal | your mark |
| --- | --- | --- | --- | --- | --- | --- |
| H1 | Moreau / Moro | 102804 / 3741 | `MORO` | French surnames | yes | |
| H2 | Laurent / Lorand | 97015 / 2082 | `LORa` | French surnames | yes | |
| H3 | Morel / Maurel | 72745 / 13598 | `MOREL` | French surnames | yes | |
| H4 | Faure / Fort | 62937 / 11160 | `FOR` | French surnames | yes | |
| H5 | Morin / Maurin | 54669 / 14722 | `MORe` | French surnames | yes | |
| H6 | Marie / Mary | 2257630 / 6905 | `MARI` | French first names | yes | |
| H7 | Jeanne / Jane | 565910 / 8550 | `JAN` | French first names | yes | |
| H8 | Philippe / Filipe | 539060 / 2535 | `FILI` | French first names | yes | |
| H9 | Paul / Pol | 429850 / 3175 | `POL` | French first names | yes | |
| H10 | Henri / Emrys | 407190 / 1465 | `aRI` | French first names | no | |
| H11 | Lewis / Louis | 531781 / 23738 | `LUIS` | English surnames | yes | |
| H12 | Mitchell / Michel | 384486 / 25578 | `MIXEL` | English surnames | no | |
| H13 | Stewart / Stuart | 324957 / 36540 | `STUART` | English surnames | yes | |
| H14 | Cruz / Crews | 334201 / 24219 | `KRUS` | English surnames | yes | |
| H15 | Tran / Trahan | 188498 / 14114 | `TRAN` | English surnames | no | |
| H16 | Mary / Marie | 4156654 / 542270 | `MARI` | English first names | yes | |
| H17 | Michael / Mikael | 4448633 / 6239 | `MIKAEL` | English first names | yes | |
| H18 | Joseph / Josef | 2680213 / 9158 | `JOSEF` | English first names | yes | |
| H19 | Christopher / Kristopher | 2079103 / 63548 | `KRISTOFER` | English first names | yes | |
| H20 | Anna / Hannah | 917615 / 466406 | `ANA` | English first names | yes | |

## 6. Misses - different keys, a usual spelling variation (do they sound the same?)

| id | pair | frequencies | keys | list | proposal | your mark |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | Richard / Ricard | 109354 / 12727 | `RIXAR` / `RIKAR` | French surnames | no | |
| M2 | Vidal / Vital | 41003 / 1835 | `VIDAL` / `VITAL` | French surnames | no | |
| M3 | Gros / Gross | 20998 / 7872 | `GRO` / `GROS` | French surnames | no | |
| M4 | Gay / Jay | 23032 / 4386 | `GE` / `JE` | French surnames | no | |
| M5 | Vallee / Valle | 19985 / 3274 | `VALE` / `VAL` | French surnames | no | |
| M6 | Jean / Jéhan | 1910930 / 1170 | `Ja` / `JEa` | French first names | no | |
| M7 | André / Andrée | 713150 / 215540 | `aDR` / `aDRE` | French first names | yes, but kept apart on purpose (man / woman) | |
| M8 | Lucas / Lukas | 186240 / 9595 | `LYKA` / `LYKAS` | French first names | no | |
| M9 | Denis / Déniz | 153235 / 2105 | `DENI` / `DENIZ` | French first names | no | |
| M10 | Loïc / Loïs | 106790 / 12610 | `LUAK` / `LUA` | French first names | no | |
| M11 | Lee / Le | 693023 / 110967 | `Le` / `LE` | English surnames | no | |
| M12 | Stewart / Steward | 324957 / 22318 | `STUART` / `STUARD` | English surnames | no | |
| M13 | Reed / Redd | 277030 / 13119 | `ReD` / `RED` | English surnames | no | |
| M14 | Chan / Khan | 76664 / 76171 | `XAN` / `KAN` | English surnames | no | |
| M15 | Garrett / Jarrett | 110697 / 21821 | `GARET` / `JARET` | English surnames | no | |
| M16 | Michelle / Michele | 819760 / 226063 | `MIXELE` / `MIXeL` | English first names | **yes** | |
| M17 | Eric / Erich | 893184 / 11706 | `ERIK` / `ERIX` | English first names | no | |
| M18 | Diane / Dianne | 520034 / 95475 | `DIaN` / `DIAN` | English first names | **yes** | |
| M19 | Kelly / Keely | 556365 / 12719 | `KELI` / `KeLI` | English first names | no | |
| M20 | Kyle / Kylee | 494713 / 50835 | `KiL` / `KILe` | English first names | no | |

M16 and M18 are the two rows where I think a rule is wrong (both are spellings of one feminine name). They are the first ones to look at.

## What I do with your marks

- You do nothing else. I turn each mark into a row of `phonetic-fr.json` or `phonetic-en.json` (`collide` for a pair that sounds the same, `differ` for one that does not), change the rules until the files pass, bump `fr-rules` to 3 and `en-rules` to 2 if a rule changed, and record it in `docs/DECISIONS.md`.
- The tables 1 to 4 also give the minimum length. With the proposals above, the pairs I think are not "did you mean" material are 2 of 16 at 4 letters, 1 of 16 at 5 letters and 4 of 16 at 6 letters or more: longer names do not make a safer suggestion, because the false ones are suffixed forms ("Georges"/"Georget", "Nicolas"/"Nicola"), not short typos. Your marks confirm or change that count; the recommendation of the report (4 letters in the three places) follows from it.
- Rows `H1` to `H20` and `M1` to `M20` that you leave unmarked are kept as I proposed and become contract rows too, so they will be tested from now on.

## Results

| row | mark | note |
| --- | --- | --- |
| (to be filled with the ids you mark) | | |

## Nothing else to run

The same numbers can be printed at any time (about 10 seconds, release build):

```text
cd apps/desktop/src-tauri
cargo test --release --test knowledge_names_calibration -- --nocapture
```

It needs the lists in `data/names/` (`python scripts/fetch_name_lists.py`, done once); without them it prints a notice and passes.
