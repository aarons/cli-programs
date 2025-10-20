# Blog Post Writing Guidelines

## Purpose

These posts document updates to CLI tools for developers who use them. The goal is to communicate changes clearly and provide necessary context.

## Language and Tone

**Do:**
- Use direct, factual language
- Use active voice with the feature/component as the subject
- Keep sentences straightforward and concise
- Provide context for why changes matter

**Avoid:**
- Personal pronouns (I, we, my, our)
- Self-referential language ("I'm excited to announce", "I've been working on")
- Marketing language or hype
- Unnecessary adjectives or adverbs
- Verbose explanations when simple ones suffice
- Passive voice constructions

### Voice and Subject

Since the tool name and version are in the section header, use the feature or component as the active subject:

**Preferred:**
```
URL detection no longer flags words ending with periods at sentence boundaries.
The installer now works from any directory.
```

**Avoid:**
```
Fixed a bug in URL detection...
The bug was fixed...
```

## Structure

### Post Format

```markdown
# CLI Tools Update - [Date]

Brief intro sentence about the update.

## [tool-name] [version]

Description of changes with context.

## [next-tool] [version]

...
```

### Headers

- Use `##` for each tool section
- Include version number in the header
- Mark new tools explicitly: `## tool-name (new tool)`

### Content Organization

- Start with what changed (the fix, feature, or improvement)
- Explain what the tool does when introducing new tools
- Describe different usage patterns or code paths if relevant
- Provide concrete examples when they add clarity

## Technical Details

**Include:**
- Specific bug descriptions with examples
- Command examples when helpful
- Brief explanations of how tools work
- Version numbers
- Links to the repository

**Omit:**
- Implementation details unless relevant to users
- Internal refactoring that doesn't affect usage
- Excessive technical jargon

## Examples

### Good

```
URL detection no longer triggers false positives for words that correspond to
TLDs when they end with a period at the end of a sentence. For example,
"this isn't it." -> "it" will no longer be flagged as a domain.
```

```
The installer now works from any directory instead of requiring execution from
the repository root.
```

### Avoid

```
I'm happy to announce that I've fixed an annoying bug that's been bothering me
for a while. The URL detection was being way too aggressive and catching false
positives everywhere!
```

```
Fixed a bug in the URL detection logic that was incorrectly flagging words...
```

## Length

- Keep posts concise
- Each tool section: 1-3 short paragraphs
- Provide enough context to understand the change
- No minimum word count requirements

## Audience

Write for developers who:
- Use these tools regularly
- Need to know what changed and why
- Want clear information without fluff
- May be scanning quickly for relevant updates
