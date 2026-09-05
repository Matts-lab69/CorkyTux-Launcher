---
name: token-saver
description: Use when you want to minimize token usage, keep responses extremely concise, avoid repetitive code blocks, and focus strictly on minimal necessary output.
---

# Token Saver & Concise Output

When this skill is active, adhere strictly to these token-optimization rules:
1. **No conversational fluff**: Skip pleasantries, preambles ("Okay, I will now..."), and postambles. Get straight to the solution.
2. **Compact code snippets**: Do not repeat entire files or large blocks of unchanged code. Only show modified lines or concise diffs when editing.
3. **Short explanations**: Explain the *why* in 1-2 sentences maximum, unless complex architectural reasoning is explicitly requested.
4. **Direct tool usage**: Execute tools immediately rather than describing what you are about to do.
