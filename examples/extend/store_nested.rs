//! Complex nested structures — the kind an Operation would produce.
//!
//! Operations expose one or more Stores as output. This example shows patterns
//! you'd use when writing an Operation that returns structured results: query
//! rows as an array of maps, grouped data as a map of arrays, and deeply nested
//! configs. All construction uses closure-based APIs and chaining with `?`.

use panopticon_core::extend::{Store, Value};

fn write_query_results(store: &mut Store) -> Result<(), Box<dyn std::error::Error>> {
    store.with_array("rows", |rows| {
        // Row 1 — all fields inserted via chaining, no .into() needed.
        let mut row = rows.push_map()?;
        row.insert("user", "alice@contoso.com")?
            .insert("ip", "10.0.0.1")?
            .insert("status", "Success")?
            .insert("attempts", 1i64)?;

        // Row 2
        let mut row = rows.push_map()?;
        row.insert("user", "bob@contoso.com")?
            .insert("ip", "192.168.1.50")?
            .insert("status", "Failure")?
            .insert("attempts", 3i64)?;

        Ok(())
    })?;

    Ok(())
}

fn write_grouped_data(store: &mut Store) -> Result<(), Box<dyn std::error::Error>> {
    store.with_map("by_severity", |groups| {
        groups.with_array("high", |arr| {
            arr.push("INC-001")?.push("INC-004")?;
            Ok(())
        })?;

        groups.with_array("medium", |arr| {
            arr.push("INC-002")?;
            Ok(())
        })?;

        groups.with_array("low", |arr| {
            arr.push("INC-003")?.push("INC-005")?.push("INC-006")?;
            Ok(())
        })?;

        Ok(())
    })?;

    Ok(())
}

fn write_nested_config(store: &mut Store) -> Result<(), Box<dyn std::error::Error>> {
    store.with_map("pipeline_config", |cfg| {
        cfg.insert("name", "signin_analysis")?;

        cfg.with_map("source", |src| {
            src.insert("type", "kql")?
                .insert("workspace", "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")?;

            src.with_map("options", |opts| {
                opts.insert("timeout_secs", 30i64)?
                    .insert("max_rows", 10_000i64)?;
                Ok(())
            })?;

            Ok(())
        })?;

        cfg.with_array("outputs", |outputs| {
            let mut o = outputs.push_map()?;
            o.insert("format", "json")?
                .insert("path", "/tmp/results.json")?;

            let mut o = outputs.push_map()?;
            o.insert("format", "csv")?
                .insert("path", "/tmp/results.csv")?;

            Ok(())
        })?;

        Ok(())
    })?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::new();

    // An Operation would call these to populate its output Store.
    write_query_results(&mut store)?;
    write_grouped_data(&mut store)?;
    write_nested_config(&mut store)?;

    // --- Reading results back via chained accessors ---

    // Query rows: array -> index -> key -> value
    let bob_status = store
        .get("rows")?
        .get_index(1)?
        .get_key("status")?
        .get_value()?;
    println!("rows[1].status = {bob_status:?}");
    assert_eq!(bob_status, &Value::Text("Failure".into()));

    let alice_attempts = store
        .get("rows")?
        .get_index(0)?
        .get_key("attempts")?
        .get_value()?;
    println!("rows[0].attempts = {alice_attempts:?}");
    assert_eq!(alice_attempts, &Value::Integer(1));

    // Grouped data: map -> key -> index -> value
    let high_count = store.get("by_severity")?.get_key("high")?.as_array()?.len();
    println!("by_severity.high: {high_count} incidents");
    assert_eq!(high_count, 2);

    let first_low = store
        .get("by_severity")?
        .get_key("low")?
        .get_index(0)?
        .get_value()?;
    println!("by_severity.low[0] = {first_low:?}");

    // Nested config: map -> key -> key -> key -> value
    let timeout = store
        .get("pipeline_config")?
        .get_key("source")?
        .get_key("options")?
        .get_key("timeout_secs")?
        .get_value()?;
    println!("pipeline_config.source.options.timeout_secs = {timeout:?}");
    assert_eq!(timeout, &Value::Integer(30));

    // Config outputs array: map -> key -> index -> key -> value
    let second_format = store
        .get("pipeline_config")?
        .get_key("outputs")?
        .get_index(1)?
        .get_key("format")?
        .get_value()?;
    println!("pipeline_config.outputs[1].format = {second_format:?}");
    assert_eq!(second_format, &Value::Text("csv".into()));

    println!("store_nested: ok");
    Ok(())
}
