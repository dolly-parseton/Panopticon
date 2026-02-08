# Core Concepts

Panopticon is built around a small set of interlocking concepts that, once understood, make the library predictable and composable. This section walks through the mental model we use when designing pipelines.

## The Big Picture

A Panopticon pipeline is a declarative data processing workflow. Rather than writing imperative code that fetches data, transforms it, and stores results, we describe *what* we want to happen and let the library figure out the execution details.

```
┌─────────────────────────────────────────────────────────────────┐
│                         Pipeline                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                      Namespaces                           │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │  │
│  │  │   "data"    │  │   "query"   │  │   "stats"   │        │  │
│  │  │   (Once)    │  │   (Once)    │  │ (Iterative) │        │  │
│  │  │  ┌───────┐  │  │  ┌───────┐  │  │  ┌───────┐  │        │  │
│  │  │  │Command│  │  │  │Command│  │  │  │Command│  │        │  │
│  │  │  └───────┘  │  │  └───────┘  │  │  └───────┘  │        │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘        │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                     Data Stores                           │  │
│  │     ScalarStore (JSON-like values)                        │  │
│  │     TabularStore (Polars DataFrames)                      │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Key Concepts

### [Pipeline State Machine](./pipeline-state-machine.md)

Pipelines progress through three states: **Draft**, **Ready**, and **Completed**. This state machine prevents common errors like modifying a running pipeline or executing an incomplete one. We use Rust's type system to enforce these transitions at compile time.

### [Namespaces](./namespaces.md)

Namespaces group related commands and control how they execute. The three namespace types - **Once**, **Iterative**, and **Static** - let us express single operations, loops over data, and constant configuration respectively.

### [Commands and Attributes](./commands-and-attributes.md)

Commands are the units of work in a pipeline. Each command has a type (like `FileCommand` or `SqlCommand`), a name, and attributes that configure its behavior. We configure commands using the `ObjectBuilder` pattern for type-safe attribute construction.

### [Data Stores](./data-stores.md)

All data flows through two stores: the **ScalarStore** for JSON-like values (strings, numbers, objects, arrays) and the **TabularStore** for Polars DataFrames. Commands read from and write to these stores using **StorePaths** - dot-separated addresses like `data.load.users.data`.

## How They Fit Together

Here is the typical flow of a Panopticon pipeline:

1. **Build** - We create a `Pipeline<Draft>`, add namespaces, and configure commands with attributes
2. **Compile** - We call `.compile()` to validate the pipeline and transition to `Pipeline<Ready>`
3. **Execute** - We call `.execute()` to run all commands, transitioning to `Pipeline<Completed>`
4. **Collect** - We call `.results()` to gather outputs from the data stores
5. **Iterate** (optional) - We call `.edit()` to return to Draft state and add more commands

This lifecycle ensures that pipelines are valid before execution and that results are only accessible after completion. The following sections dive deeper into each concept.
