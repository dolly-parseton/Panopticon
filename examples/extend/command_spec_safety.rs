//! Example: CommandSpec builder safety and edge cases
//!
//! Demonstrates correct and incorrect use of the CommandSpecBuilder, including:
//!   - The LiteralFieldRef compile-time safety mechanism
//!   - NamePolicy violations (reserved names, forbidden characters)
//!   - Builder validation (invalid derived result references)
//!   - Mixed literal/template fields on the same ObjectFields
//!
//! Run with: cargo run --example command_spec_safety

use panopticon_core::extend::*;
use std::panic;

fn main() {
    // ═══════════════════════════════════════════════════════════════════════
    // 1. VALID SPEC: derived_result with LiteralFieldRef
    // ═══════════════════════════════════════════════════════════════════════
    //
    // The correct pattern: add_literal() returns a LiteralFieldRef that can
    // be passed to derived_result(). This proves at compile time that the
    // result name comes from a literal (non-template) field.

    println!("=== 1. Valid derived_result with LiteralFieldRef ===\n");
    {
        let (pending, fields) =
            CommandSpecBuilder::new().array_of_objects("items", true, Some("Array of named items"));

        // add_literal returns (ObjectFields, LiteralFieldRef)
        let (fields, name_ref) = fields.add_literal(
            "name",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Unique item name — safe for derived results"),
        );

        let (attrs, results) = pending
            .finalise_attribute(fields)
            .derived_result("items", name_ref, None, ResultKind::Data)
            .build();

        println!(
            "  Built {} attribute(s), {} result(s)",
            attrs.len(),
            results.len()
        );
        println!("  OK: derived_result accepted the LiteralFieldRef\n");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // 2. COMPILE-TIME SAFETY: template fields cannot produce LiteralFieldRef
    // ═══════════════════════════════════════════════════════════════════════
    //
    // add_template() returns Self — NOT a (Self, LiteralFieldRef) tuple.
    // This means you literally cannot pass a template field to derived_result()
    // because no LiteralFieldRef exists for it. The compiler catches this.

    println!("=== 2. Template fields: no LiteralFieldRef available ===\n");
    {
        let (pending, fields) = CommandSpecBuilder::new().array_of_objects("items", true, None);

        let (fields, name_ref) =
            fields.add_literal("name", TypeDef::Scalar(ScalarType::String), true, None);

        // add_template returns just ObjectFields — no LiteralFieldRef
        let fields = fields.add_template(
            "value",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Tera template — resolved at runtime"),
            ReferenceKind::StaticTeraTemplate,
        );

        // WON'T COMPILE — uncomment to see the error:
        //
        //   let value_ref: LiteralFieldRef<&str> = ???;
        //   // There is no way to obtain a LiteralFieldRef for "value" because
        //   // add_template() doesn't return one. The type system prevents it.
        //   pending.finalise_attribute(fields).derived_result("items", value_ref, ...);
        //
        // The only LiteralFieldRef we have is name_ref (from the "name" literal field).

        let (_attrs, _results) = pending
            .finalise_attribute(fields)
            .derived_result("items", name_ref, None, ResultKind::Data)
            .build();

        println!("  add_template(\"value\", ...) returned ObjectFields (no LiteralFieldRef)");
        println!("  add_literal(\"name\", ...) returned (ObjectFields, LiteralFieldRef)");
        println!("  derived_result() can only use name_ref — compiler enforces this\n");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // 3. NAMEPOLICY VIOLATIONS (panic at LazyLock / build time)
    // ═══════════════════════════════════════════════════════════════════════
    //
    // The DEFAULT_NAME_POLICY rejects:
    //   - Reserved names: "item", "index" (used by iterative namespaces)
    //   - Forbidden characters: anything not [a-zA-Z0-9_]

    println!("=== 3. NamePolicy violations ===\n");

    // 3a. Reserved attribute name "item"
    let result = panic::catch_unwind(|| {
        CommandSpecBuilder::<&str>::new()
            .attribute(
                AttributeSpecBuilder::new("item", TypeDef::Scalar(ScalarType::String))
                    .required()
                    .build(),
            )
            .build();
    });
    println!("  attribute(\"item\"): {}", format_panic_result(&result));

    // 3b. Reserved field name "index"
    let result = panic::catch_unwind(|| {
        let fields = ObjectFields::<&str>::new();
        let (fields, _) =
            fields.add_literal("index", TypeDef::Scalar(ScalarType::Number), true, None);
        fields.build();
    });
    println!("  field(\"index\"):    {}", format_panic_result(&result));

    // 3c. Forbidden characters: dots
    let result = panic::catch_unwind(|| {
        CommandSpecBuilder::<&str>::new()
            .fixed_result(
                "bad.name",
                TypeDef::Scalar(ScalarType::String),
                None,
                ResultKind::Data,
            )
            .build();
    });
    println!("  result(\"bad.name\"): {}", format_panic_result(&result));

    // 3d. Forbidden characters: spaces
    let result = panic::catch_unwind(|| {
        CommandSpecBuilder::<&str>::new()
            .attribute(
                AttributeSpecBuilder::new("my field", TypeDef::Scalar(ScalarType::String))
                    .required()
                    .build(),
            )
            .build();
    });
    println!(
        "  attribute(\"my field\"): {}",
        format_panic_result(&result)
    );

    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // 4. BUILDER VALIDATION (panic at build time)
    // ═══════════════════════════════════════════════════════════════════════
    //
    // CommandSpecBuilder.build() verifies that derived results reference
    // valid attributes with the correct structure.

    println!("=== 4. Builder validation panics ===\n");

    // 4a. derived_result references a nonexistent attribute
    let result = panic::catch_unwind(|| {
        let (pending, fields) = CommandSpecBuilder::new().array_of_objects("things", true, None);
        let (fields, name_ref) =
            fields.add_literal("name", TypeDef::Scalar(ScalarType::String), true, None);

        pending
            .finalise_attribute(fields)
            // "nonexistent" doesn't match any attribute — "things" is the actual name
            .derived_result("nonexistent", name_ref, None, ResultKind::Data)
            .build();
    });
    println!(
        "  derived_result(\"nonexistent\", ...): {}",
        format_panic_result(&result)
    );

    // 4b. derived_result references a scalar attribute (not ArrayOf(ObjectOf))
    let result = panic::catch_unwind(|| {
        // We need a LiteralFieldRef to even call derived_result.
        // Create one from a legitimate array_of_objects, then misuse it.
        let (pending, fields) =
            CommandSpecBuilder::new().array_of_objects("valid_array", true, None);
        let (fields, name_ref) =
            fields.add_literal("name", TypeDef::Scalar(ScalarType::String), true, None);

        pending
            .finalise_attribute(fields)
            // Add a plain scalar attribute
            .attribute(
                AttributeSpecBuilder::new("scalar_attr", TypeDef::Scalar(ScalarType::String))
                    .required()
                    .build(),
            )
            // Try to derive results from the scalar attribute — not ArrayOf(ObjectOf)
            .derived_result("scalar_attr", name_ref, None, ResultKind::Data)
            .build();
    });
    println!(
        "  derived_result(\"scalar_attr\", ...): {}",
        format_panic_result(&result)
    );

    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // 5. MIXED LITERAL + TEMPLATE FIELDS
    // ═══════════════════════════════════════════════════════════════════════
    //
    // A single ObjectFields can contain both literal and template fields.
    // Only literals yield LiteralFieldRef handles.

    println!("=== 5. Mixed literal and template fields ===\n");
    {
        let (pending, fields) = CommandSpecBuilder::new().array_of_objects(
            "transforms",
            true,
            Some("Array of {name, expression, description} transform specs"),
        );

        // Literal: safe for derived result names
        let (fields, name_ref) = fields.add_literal(
            "name",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Transform identifier (literal — yields LiteralFieldRef)"),
        );

        // Template: supports Tera substitution at pipeline execution time
        let fields = fields.add_template(
            "expression",
            TypeDef::Scalar(ScalarType::String),
            true,
            Some("Tera expression to evaluate (template — no LiteralFieldRef)"),
            ReferenceKind::StaticTeraTemplate,
        );

        // Another literal: also yields a LiteralFieldRef
        let (fields, _desc_ref) = fields.add_literal(
            "description",
            TypeDef::Scalar(ScalarType::String),
            false,
            Some("Human-readable description (literal — also yields LiteralFieldRef)"),
        );

        let (attrs, results) = pending
            .finalise_attribute(fields)
            // Use the "name" literal field for derived results
            .derived_result("transforms", name_ref, None, ResultKind::Data)
            .build();

        println!("  Fields: name (literal), expression (template), description (literal)");
        println!("  LiteralFieldRefs obtained: name_ref, _desc_ref");
        println!("  derived_result uses name_ref — template field \"expression\" cannot be used");
        println!(
            "  Built {} attribute(s), {} result(s)",
            attrs.len(),
            results.len()
        );
    }

    println!("\nAll examples completed.");
}

fn format_panic_result<T>(
    result: &std::result::Result<T, Box<dyn std::any::Any + Send>>,
) -> String {
    match result {
        Ok(_) => "OK (no panic)".to_string(),
        Err(payload) => {
            if let Some(msg) = payload.downcast_ref::<String>() {
                format!("CAUGHT PANIC: {}", msg)
            } else if let Some(msg) = payload.downcast_ref::<&str>() {
                format!("CAUGHT PANIC: {}", msg)
            } else {
                "CAUGHT PANIC: (non-string payload)".to_string()
            }
        }
    }
}
