# Extension Architecture

Before implementing your first custom command, it helps to understand the mental model behind Panopticon's extension system. This chapter provides the foundational understanding you need to work effectively with the framework.

## The Big Picture

Panopticon separates **what a command is** from **how it runs**. This separation enables:

- **Compile-time validation** of command specifications before any code executes
- **Type safety** for attribute parsing and result declarations
- **Introspection** of available commands without instantiating them
- **Dependency analysis** across the pipeline before execution begins

At the heart of this design are three traits that every command must implement:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                           The Three-Trait Model                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────┐    ┌─────────────────┐    ┌──────────────┐               │
│   │ Descriptor  │    │ FromAttributes  │    │  Executable  │               │
│   ├─────────────┤    ├─────────────────┤    ├──────────────┤               │
│   │ "What am I?"│    │ "How am I       │    │ "What do I   │               │
│   │             │    │  constructed?"  │    │  actually    │               │
│   │ - type name │    │                 │    │  do?"        │               │
│   │ - attributes│    │ - parse attrs   │    │              │               │
│   │ - results   │    │ - extract deps  │    │ - execute()  │               │
│   └─────────────┘    └─────────────────┘    └──────────────┘               │
│          │                   │                     │                        │
│          └───────────────────┼─────────────────────┘                        │
│                              │                                              │
│                              ▼                                              │
│                    ┌─────────────────┐                                      │
│                    │     Command     │  ← Blanket impl: auto-implemented   │
│                    │   (marker trait)│    for any T: Descriptor +          │
│                    └─────────────────┘    FromAttributes + Executable      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## The Three Traits

### Descriptor: Declaring Your Command's Shape

The `Descriptor` trait declares static metadata about your command. Think of it as the command's "schema" - it tells the framework:

- **What type of command is this?** (a unique identifier string)
- **What attributes does it accept?** (inputs with types, hints, and requirements)
- **What results does it produce?** (outputs with types and kinds)

```rust
pub trait Descriptor: Sized {
    fn command_type() -> &'static str;
    fn command_attributes() -> &'static [AttributeSpec<&'static str>];
    fn command_results() -> &'static [ResultSpec<&'static str>];

    // Default implementations combine your specs with common attributes/results
    fn available_attributes() -> Vec<&'static AttributeSpec<&'static str>>;
    fn available_results() -> Vec<&'static ResultSpec<&'static str>>;
}
```

The framework automatically extends your declared attributes with common ones like `when` (conditional execution), and extends your results with metadata like `status` and `duration_ms`.

### FromAttributes: Parsing Configuration

The `FromAttributes` trait handles construction from a hashmap of attribute values. It bridges the gap between the raw YAML/JSON configuration and your typed command struct.

```rust
pub trait FromAttributes: Sized + Descriptor {
    fn from_attributes(attrs: &Attributes) -> Result<Self>;

    // Default: uses Descriptor's spec to scan for template references
    fn extract_dependencies(attrs: &Attributes) -> Result<HashSet<StorePath>>;

    // Default: returns a factory function that validates + constructs
    fn factory() -> CommandFactory;
}
```

The `factory()` method is particularly important - it produces a `CommandFactory` that the framework stores and calls at runtime. This factory:

1. Validates the provided attributes against your declared spec
2. Extracts the `when` condition (if present) for conditional execution
3. Calls your `from_attributes()` to construct the command instance
4. Wraps the result in an `ExecutableWrapper` that handles common behavior

### Executable: Doing the Work

The `Executable` trait is where your command's actual logic lives. It receives the runtime context and a path where results should be stored.

```rust
#[async_trait]
pub trait Executable: Send + Sync + 'static {
    async fn execute(
        &self,
        context: &ExecutionContext,
        output_prefix: &StorePath
    ) -> Result<()>;
}
```

The `ExecutionContext` provides:
- Access to the scalar and tabular stores (read previous results)
- Template substitution via Tera
- Extension points for custom services
- Cancellation checks

## The Command Trait: Automatic Composition

You never implement `Command` directly. Instead, it is automatically implemented for any type that satisfies all three constituent traits:

```rust
pub trait Command: FromAttributes + Descriptor + Executable {}

// Blanket implementation - you get this for free
impl<T: FromAttributes + Descriptor + Executable> Command for T {}
```

This design means the compiler enforces completeness: you cannot accidentally forget to implement one of the required traits.

## CommandFactory and Registration

When you add a command to a pipeline, Panopticon does not immediately construct your command. Instead, it stores a `CommandFactory`:

```rust
pub type CommandFactory = Box<dyn Fn(&Attributes) -> Result<Box<dyn Executable>>>;
```

This factory is a closure that captures everything needed to construct your command later, at execution time. The flow looks like this:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Command Registration and Execution                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  COMPILE TIME (Pipeline::add_command)                                       │
│  ─────────────────────────────────────                                      │
│                                                                             │
│  ┌──────────────┐      ┌─────────────────────┐      ┌──────────────────┐   │
│  │ Your Command │ ──▶  │ T::factory()        │ ──▶  │   CommandSpec    │   │
│  │   Type (T)   │      │ (creates closure)   │      │                  │   │
│  └──────────────┘      └─────────────────────┘      │ - namespace_idx  │   │
│        │                                             │ - name           │   │
│        │ T::command_attributes()                     │ - attributes     │   │
│        │ T::command_results()                        │ - factory ◀──────┤   │
│        │ T::extract_dependencies()                   │ - dependencies   │   │
│        ▼                                             │ - expected specs │   │
│  ┌─────────────────────┐                            └──────────────────┘   │
│  │ Static Spec Data    │ ◀───────────────────────────────────┘             │
│  │ (attributes/results)│                                                    │
│  └─────────────────────┘                                                    │
│                                                                             │
│                                                                             │
│  RUNTIME (Pipeline::execute)                                                │
│  ────────────────────────────                                               │
│                                                                             │
│  ┌──────────────────┐      ┌─────────────────────┐     ┌────────────────┐  │
│  │   CommandSpec    │ ──▶  │ factory(&attrs)     │ ──▶ │ Box<dyn        │  │
│  │  .factory(...)   │      │                     │     │   Executable>  │  │
│  └──────────────────┘      │ 1. validate attrs   │     └────────────────┘  │
│                            │ 2. from_attributes()│            │            │
│                            │ 3. wrap in handler  │            ▼            │
│                            └─────────────────────┘     ┌────────────────┐  │
│                                                        │   .execute()   │  │
│                                                        └────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

This deferred construction is essential because:

1. **Attributes may contain templates** that reference values not yet computed
2. **Iterative namespaces** create multiple command instances with different `item` values
3. **Validation happens early**, before any expensive work

## Compile-Time vs Runtime Separation

A key insight of this architecture is the clean separation between compile-time specification and runtime execution:

| Aspect | Compile Time | Runtime |
|--------|-------------|---------|
| **When** | `Pipeline::compile()` | `Pipeline::execute()` |
| **What's known** | Attribute specs, result specs, dependencies | Actual attribute values, context state |
| **Type info** | Full generic types (`T: Command`) | Type-erased (`Box<dyn Executable>`) |
| **Validation** | Schema validation, dependency graph, naming | Value parsing, template resolution |
| **Fails if** | Invalid specs, circular deps, reserved names | Bad template, missing dependency, execution error |

This separation provides several benefits:

### Early Error Detection

Problems with your command specification surface immediately when the pipeline is compiled, not when that particular command happens to run. A typo in an attribute name, a reserved field, or a circular dependency all fail fast.

### Safe Type Erasure

At compile time, the framework has access to your full concrete type `T`. It extracts everything it needs (specs, factory function, dependencies) and stores them in a type-erased `CommandSpec`. At runtime, only the `Box<dyn Executable>` interface is needed.

### Dependency Analysis

Before execution begins, the framework can analyze all dependencies between commands. The `extract_dependencies()` method scans your attributes for store path references, enabling the execution planner to order commands correctly and detect cycles.

## Why This Architecture?

The three-trait model might seem like more ceremony than a simpler "just implement `execute()`" approach. Here is why the additional structure pays off:

### Type Safety for Specs

By requiring specs as static data (`&'static [AttributeSpec]`), the framework ensures your command's schema is:
- Known at compile time
- Immutable at runtime
- Efficiently comparable without heap allocation

The `CommandSpecBuilder` with `LazyLock` provides a convenient pattern for constructing these specs once on first access.

### Compile-Time Guarantees for Derived Results

Some commands produce dynamic results based on attribute values (e.g., an "export" command where each item in an array becomes a separate output). The `LiteralFieldRef` mechanism ensures these derived result names can only come from literal (non-template) fields - templates could produce unpredictable names at runtime.

### Testability

Each trait can be tested in isolation:
- `Descriptor`: verify your specs are correct
- `FromAttributes`: verify parsing handles edge cases
- `Executable`: verify behavior given a mocked context

### Extensibility

The trait-based approach enables custom commands without modifying Panopticon's core. Your types implement the same traits as built-in commands, receiving identical treatment from the framework.

## Summary

The extension architecture revolves around three traits:

- **Descriptor** declares what your command accepts and produces (static metadata)
- **FromAttributes** parses configuration into your command struct (construction)
- **Executable** performs the actual work (runtime behavior)

The **Command** marker trait is auto-implemented when you satisfy all three. The **CommandFactory** type enables deferred construction, allowing compile-time validation before runtime execution.

This separation between specification and execution enables early error detection, safe type erasure, and comprehensive dependency analysis - all of which contribute to more reliable pipeline execution.

---

Next: [Your First Custom Command](../first-command/index.md) - Put this knowledge into practice by building a working command from scratch.
