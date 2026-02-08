# Advanced Patterns

This section covers expert-level patterns for library extenders who need to push beyond the basics. These patterns emerge from real-world requirements: sharing state across commands, handling dynamic result names safely, and building robust error handling into your commands.

## When You Need These Patterns

The patterns in this section solve specific problems:

| Pattern | Use When |
|---------|----------|
| [Type-Indexed Extensions](./type-indexed-extensions.md) | Commands need shared state: HTTP clients, database pools, auth tokens |
| [Derived Results](./derived-results.md) | Result names come from user input, not hardcoded strings |
| [Error Handling](./error-handling.md) | Building production-quality commands with proper error context |

## Prerequisites

Before diving into these patterns, you should be comfortable with:

- Building custom commands (the three traits: `Descriptor`, `FromAttributes`, `Executable`)
- The spec system (`CommandSpecBuilder`, `AttributeSpecBuilder`, `ObjectFields`)
- Working with `ExecutionContext` and `InsertBatch`

If any of these feel unfamiliar, review the earlier sections first.

## Pattern Complexity vs. Necessity

Not every command needs advanced patterns. Consider the tradeoffs:

```text
Simple Command (hardcoded results, no shared state):
  └── Use fixed_result(), implement the three traits, done.

Medium Complexity (dynamic iteration, basic error handling):
  └── Use array_of_objects with derived_result(), add context() to errors.

Complex Command (shared HTTP client, auth tokens, robust errors):
  └── Use Extensions for state, LiteralFieldRef for safety, full anyhow patterns.
```

## The Extension Ecosystem

These patterns work together. A real-world API integration command might use all three:

```rust
// Type-indexed extension: shared HTTP client
let client = context.extensions()
    .read().await
    .get::<reqwest::Client>()
    .cloned()
    .ok_or_else(|| anyhow::anyhow!("HTTP client not configured"))?;  // Error handling

// Derived results: endpoint names from user config
.derived_result("endpoints", name_ref, None, ResultKind::Data)  // Safe via LiteralFieldRef
```

## What's Next

Start with whichever pattern matches your immediate need:

1. **[Type-Indexed Extensions](./type-indexed-extensions.md)** - If you need to share expensive resources (clients, connections, tokens) across commands without global state.

2. **[Derived Results](./derived-results.md)** - If your command produces results whose names aren't known until the user provides configuration.

3. **[Error Handling](./error-handling.md)** - If you want to understand the error patterns used throughout Panopticon's built-in commands.

Each section includes real-world examples, anti-patterns to avoid, and guidance on when the pattern is (and isn't) appropriate.
