## Context

Live reads show 14 Cairn changes, 35 Radicle issues, 56 patches and 54 jobs. The other five registered Cairn types are absent from the live stores. An issue titled `Custom styling support` renders as `Create issue`; its title is in the root's second action. Cairn init actions contain named text artifacts; job actions use single-key Request/Run/Finished variants rather than a `type` discriminator.

## Decisions

### Decision: Observe root metadata, never choose a lifecycle winner

Read title and excerpt from bounded root action blobs, including later numbered actions, with a deterministic root-summary fallback. Do not inspect later operations to invent a current title or status. Preserve native IDs, exact operation payloads and bounded reads. Display all history operations in their existing identity order; every status, vote and job outcome is a recorded claim, not a reduced result.

### Decision: Render content before framing

Use the existing DOM-only element helpers and facts styles. Select presentation by admitted COB type and recorded action discriminator, including externally tagged job events. Show body/artifact/field text with preserved whitespace, not JSON escape sequences. Keep producer fields as labelled recorded facts and exact JSON in per-action disclosures; collapse framing details by default. Unknown or malformed shapes visibly fall back to raw content. No source-supplied HTML, executable links, images, Markdown execution or external fetches.

## Risks / Trade-offs

A root title can differ from later edits; label it as recorded root content. Deep or future data shapes remain in exact JSON, not silently normalized away. Browser verification will use candidate assets and real read-only APIs; missing live types require explicitly labelled producer-shaped verification fixtures, not fabricated live evidence.

## Verification Strategy

Admit proposal/design/tasks before implementation. Reproduce generic live rendering, then verify real change/issue/patch/job content, all six Cairn producer shapes, unknown/null/scalar payloads, concurrent histories, inert hostile text, session clearing, selection restoration and mobile layout. Run focused service tests, JavaScript syntax and Clippy. A behavior regression must cover title in the second root action, malformed/oversized fallback and unchanged exact detail history.
