# Documentation rules

## Standard

Use ASD-STE100 Simplified Technical English (STE) for new technical
documentation and task summaries. This rule also applies to release notes
and new architecture decision records (ADRs).

Use the official ASD-STE100 standard as the language reference. The
[STEMG website](https://www.asd-ste100.org/) identifies Issue 9, dated January 15,
2025. The [official download page](https://www.asd-ste100.org/STE_downloads.html)
has the request form for a free copy.

The full standard and its dictionary are not in this repository. The rules
below help authors write clear text. They do not replace the standard.
Do not state that a document passed an ASD-STE100 check unless an actual
review used the standard. Record the review method and result when available.
If the standard is not available, write the document and record that its
full language review is pending. This does not prevent a task transfer.

## Writing rules

- Use short sentences. Give each sentence one topic.
- Use active voice when possible. Identify the person or component that acts.
- Give one instruction in each step. Use numbered steps for procedures.
- Use the same term for the same object or action.
- Use approved words with their approved meanings and parts of speech.
  Check them against the official dictionary when it is available.
- Use specific names. Replace unclear pronouns with the component name.
- Keep instructions separate from descriptions and expected results.
- State facts, decisions, results and open work separately.
- Keep paths, commands, identifiers, API names and error messages exact.
- Use project technical names and technical verbs when needed.
  Define new terms in the document or in the table below.
- Include the command and its result when you describe a check.
  Identify checks that failed or did not run.

## Project terms

| Term | Meaning in this project |
| --- | --- |
| branch | A Git reference for a line of development. |
| commit | A saved Git change with an identifier. |
| worktree | A separate Git checkout with its own working files and index. |
| pull request (PR) | A GitHub request to review and merge branch changes. |
| target branch | The branch that receives the agreed changes. |
| task summary | A file that records task facts for another chat or model. |
| release manager | The chat that integrates changes and publishes releases. |
| migration | A versioned database schema change. |
| CI | Continuous integration checks in GitHub Actions. |
| merge | Combine Git changes in the destination branch. |
| push | Send Git commits or tags to the remote repository. |

These are project terms. This table is not an ASD-STE100 dictionary approval.

## Language scope

English technical documents are the source documents. German user documents
and UI text remain German translations. User-facing chat prompts can use
German. This exception does not apply to task summaries.

Do not translate all old documents during an unrelated task. Apply these
rules to new English text and to English text that the task changes.
