// LLM prompt templates

use std::sync::LazyLock;

pub static SYSTEM_PROMPT: LazyLock<String> = LazyLock::new(|| {
    "You are an experienced software engineer that writes clear and concise Conventional Commit git commit messages.".to_string()
});

pub fn generate_commit_prompt(context: &str) -> String {
    format!(
        r#"Please write a clear message that describes the changes in this pull request.

Requirements:
- It needs to be a functionally descriptive message that will help engineers understand what is changing.
- It needs to be formatted as a Conventional Commit.
- It should have an appropriate level of detail:
  - if it's a simple change, then a single one-line message is fine
  - if it's more comprehensive, then more context and high level details are appropriate

Important:
- Work on the message draft until it is very concise and clear. Brevity with clarity is difficult to achieve but is important to strive for.

Conventional Commits have these core types:

- fix: patches a bug (correlates with PATCH in semantic versioning)
- feat: introduces a new feature (correlates with MINOR in semantic versioning)
- build: changes to build system or dependencies
- chore: routine tasks, maintenance, etc.
- ci: changes to CI configuration
- docs: documentation only changes
- style: formatting, missing semicolons, etc. (no code change)
- refactor: code change that neither fixes a bug nor adds a feature
- perf: improves performance
- test: adding or correcting tests

The key rules for a conventional commit formatted message:

1. Start with a type
2. Use a colon and space after type
3. Provide a short, descriptive summary in the first line
4. Optional body should be separated by a blank line
5. Optional footers should be separated by a blank line

Breaking changes correlate with MAJOR in semantic versioning. Mark breaking changes with either:
- Adding "!" before the colon, or
- Adding "BREAKING CHANGE:" in the footer

If anything is ambiguous; just stick to apparent facts, and do not make suppositions.
Previous commit messages have been provided for additional context.

Format your return message like this:

<observations>
Observations about the code that help plan out a clear message
Iterations on the message until it is clear and concise
</observations>
<commit_message>
commit-type: a description of the commit

Some more context about what changed.
</commit_message>

Here are the code changes:

{}
"#,
        context
    )
}

pub fn fix_message_format(original_prompt: &str, previous_response: &str) -> String {
    format!(
        r#"Please update your response. Here are the original instructions:

{}

The previous response did not follow the required format. You MUST include both observation and commit_message sections.

Previous response:
{}

Please submit a corrected version. As a reminder, it must follow this format:

<observations>
Planning, observations, and message iterations go here
Even if the commit is a single line, ensure that this section is present
</observations>
<commit_message>
commit-type: a functional description of the commit

Additional context about the commit if needed
</commit_message>
"#,
        original_prompt, previous_response
    )
}

pub fn fix_message_content(message: &str) -> String {
    format!(
        r#"Please update this commit message by removing all:

- URLs (http/https links)
- Email addresses
- Co-Authored-By or 'Generated with' attribution statements
- Emojis
- Codefences or literal code
- Any other metadata that shouldn't be in a commit message

Keep the core commit message intact and maintain proper conventional commit formatting.
IMPORTANT: Return only the cleaned commit message. Do not add formatting (such as code fences) or other explanations.

Commit message to clean:

{}"#,
        message
    )
}
