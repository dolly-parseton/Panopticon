# LiteralFieldRef: Compile-Time Safety

`LiteralFieldRef` is an opaque handle that proves a field has `ReferenceKind::Unsupported` (i.e., contains literal data). This is the key mechanism that enables compile-time safety for derived results.

## The Problem LiteralFieldRef Solves

Derived results in Panopticon use field values as result names. Consider:

```yaml
my_command:
  columns:
    - name: "total"
      expression: "{{ item.a + item.b }}"
    - name: "average"
      expression: "{{ item.sum / item.count }}"
```

This produces results named `"total"` and `"average"`. The `name` field values become keys in the output.

But what if `name` were a template?

```yaml
my_command:
  columns:
    - name: "{{ item.column_name }}"  # Template!
      expression: "{{ item.value }}"
```

The result names would depend on **runtime data**, making it impossible to know them at pipeline definition time. This breaks dependency tracking and type safety.

## The Solution: Proof of Literalness

`LiteralFieldRef` is a compile-time proof that a field cannot contain templates. The type system enforces this:

```rust
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct LiteralFieldRef<T: Into<String>> {
    name: T,  // Private field!
}
```

The `name` field is **private**. You cannot construct a `LiteralFieldRef` directly. The only way to obtain one is through `ObjectFields::add_literal()`:

```rust
impl<T: Into<String> + Clone> ObjectFields<T> {
    pub fn add_literal(
        mut self,
        name: T,
        ty: TypeDef<T>,
        required: bool,
        hint: Option<T>,
    ) -> (Self, LiteralFieldRef<T>) {
        // Creates field with ReferenceKind::Unsupported
        // Returns the proof handle
        let handle = LiteralFieldRef { name: name.clone() };
        self.fields.push(FieldSpec {
            name,
            ty,
            required,
            hint,
            reference_kind: ReferenceKind::Unsupported,  // Always literal
        });
        (self, handle)
    }
}
```

## How It Prevents Errors

`derived_result()` requires a `LiteralFieldRef`:

```rust
pub fn derived_result(
    mut self,
    attribute: T,
    name_field: LiteralFieldRef<T>,  // Must have this proof
    ty: Option<TypeDef<T>>,
    kind: ResultKind,
) -> Self
```

Since `add_template()` does not return a `LiteralFieldRef`:

```rust
pub fn add_template(
    self,
    name: T,
    ty: TypeDef<T>,
    required: bool,
    hint: Option<T>,
    kind: ReferenceKind,
) -> Self  // No LiteralFieldRef!
```

...you **cannot** pass a template field to `derived_result()`. The compiler will not allow it because there is no `LiteralFieldRef` for that field.

## Demonstration

### Valid Usage

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("items", true, None);

// add_literal returns (ObjectFields, LiteralFieldRef)
let (fields, name_ref) = fields.add_literal(
    "name",
    TypeDef::Scalar(ScalarType::String),
    true,
    None
);

// name_ref proves "name" is literal - can use it for derived_result
let (attrs, results) = pending
    .finalise_attribute(fields)
    .derived_result("items", name_ref, None, ResultKind::Data)
    .build();
```

### Attempting Invalid Usage

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("items", true, None);

// add_template returns just ObjectFields
let fields = fields.add_template(
    "computed_name",
    TypeDef::Scalar(ScalarType::String),
    true,
    None,
    ReferenceKind::RuntimeTeraTemplate
);

// There is no LiteralFieldRef for "computed_name"
// This will NOT compile:
//
// let (attrs, results) = pending
//     .finalise_attribute(fields)
//     .derived_result("items", ???, None, ResultKind::Data)
//                              ^^^
//                     No LiteralFieldRef exists for the template field!
//     .build();
```

The error is caught at compile time, not runtime.

## The Type-Level Guarantee

This pattern provides a type-level guarantee:

1. `LiteralFieldRef<T>` can only be created inside the `spec` module (private constructor)
2. The only public way to obtain one is `ObjectFields::add_literal()`
3. `add_literal()` always sets `ReferenceKind::Unsupported`
4. Therefore, any `LiteralFieldRef` you have **proves** the field is literal

This is not a convention or documentation - it is enforced by Rust's type system and module privacy.

## Accessing the Name

You can read the field name from a `LiteralFieldRef`:

```rust
impl<T: Into<String> + Clone> LiteralFieldRef<T> {
    pub fn name(&self) -> &T {
        &self.name
    }
}
```

This is used by `build()` to verify that the referenced field actually exists in the attribute.

## Build-Time Validation

While compile-time safety prevents using template fields for derived results, build-time validation catches other errors:

```rust
let (pending, fields) = CommandSpecBuilder::new()
    .array_of_objects("things", true, None);

let (fields, name_ref) = fields.add_literal("name", ...);

pending
    .finalise_attribute(fields)
    // Wrong attribute name - "items" doesn't exist, we have "things"
    .derived_result("items", name_ref, None, ResultKind::Data)
    .build();  // Panics: "Derived result references unknown attribute 'items'"
```

The builder verifies:

1. The referenced attribute exists
2. The attribute is `ArrayOf(ObjectOf { ... })`
3. The `LiteralFieldRef` name matches a field in those objects

## Why This Pattern Matters

Without `LiteralFieldRef`, you could make this mistake:

```rust
// Hypothetical unsafe API (not how Panopticon works)
builder.derived_result("items", "template_field", ...)
```

This would compile but fail at runtime when template evaluation produces unpredictable names.

With `LiteralFieldRef`:

```rust
// Safe API (actual Panopticon)
builder.derived_result("items", name_ref, ...)
//                              ^^^^^^^^
//                     Must be a LiteralFieldRef
//                     Can only come from add_literal()
//                     Guarantees field is literal
```

The error is impossible to make because the type system does not allow it.

## Multiple LiteralFieldRefs

You can have multiple literal fields and thus multiple `LiteralFieldRef` handles:

```rust
let (fields, name_ref) = fields.add_literal("name", ...);
let (fields, key_ref) = fields.add_literal("key", ...);
let (fields, id_ref) = fields.add_literal("id", ...);
```

You choose which one to use for `derived_result()`. Each handle proves its respective field is literal.

## Conversion

`LiteralFieldRef<&'static str>` can convert to `LiteralFieldRef<String>`:

```rust
impl From<LiteralFieldRef<&'static str>> for LiteralFieldRef<String> {
    fn from(r: LiteralFieldRef<&'static str>) -> Self {
        LiteralFieldRef {
            name: r.name.into(),
        }
    }
}
```

This happens automatically when building `CommandSpec` from static specs.

## Summary

`LiteralFieldRef` is an example of "making invalid states unrepresentable" through the type system:

| Want to do | Required | How to get it |
|------------|----------|---------------|
| Use derived results | `LiteralFieldRef` | `add_literal()` |
| Use template fields | Just `ObjectFields` | `add_template()` |

The asymmetry is intentional: derived result names must be predictable, so only literal fields qualify. The type system enforces this at compile time.
