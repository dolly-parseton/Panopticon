use panopticon_core::{params, prelude::*};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Basic iter_map: iterate over key-value pairs ──
    println!("=== Basic iter_map ===");
    {
        let mut pipe = Pipeline::default();
        pipe.map("config")?
            .insert("host", "localhost")?
            .insert("port", "8080")?
            .insert("protocol", "https")?;

        pipe.iter_map(
            "read_config",
            IterSource::map("config"),
            |key, value, body| {
                body.step::<SetVar>(
                    "capture_key",
                    params!(
                        "name" => "current_key",
                        "value" => Param::reference(key),
                    ),
                )?;
                body.step::<SetVar>(
                    "capture_value",
                    params!(
                        "name" => "current_value",
                        "value" => Param::reference(value),
                    ),
                )?;
                Ok(())
            },
        )?;

        pipe.hook(Logger::new().writer(std::io::stdout()));
        pipe.hook(Timeout::new(Duration::from_secs(5)));

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── iter_map with parent variable references ──
    println!("\n=== iter_map referencing parent vars ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("env", "production")?;
        pipe.map("settings")?
            .insert("db_host", "db.example.com")?
            .insert("cache_host", "cache.example.com")?;

        pipe.iter_map(
            "apply_env",
            IterSource::map("settings"),
            |key, value, body| {
                body.step::<SetVar>(
                    "label",
                    params!(
                        "name" => "entry",
                        "value" => Param::template(vec![
                            Param::reference("env"),
                            Param::literal("."),
                            Param::reference(key),
                            Param::literal("="),
                            Param::reference(value),
                        ]),
                    ),
                )?;
                Ok(())
            },
        )?;

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── iter_map followed by regular steps ──
    println!("\n=== iter_map then regular steps ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("status", "ready")?;
        pipe.map("headers")?
            .insert("content-type", "application/json")?
            .insert("accept", "text/html")?;

        pipe.iter_map(
            "scan_headers",
            IterSource::map("headers"),
            |_key, value, body| {
                body.step::<SetVar>(
                    "save",
                    params!(
                        "name" => "last_header",
                        "value" => Param::reference(value),
                    ),
                )?;
                Ok(())
            },
        )?;

        // Regular step after the iteration
        pipe.step::<SetVar>(
            "finalize",
            params!(
                "name" => "done",
                "value" => Param::reference("status"),
            ),
        )?;

        pipe.returns(
            "result",
            params!(
                "status" => Param::reference("done"),
            ),
        )?;

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── Error: iter_map with unresolved source ──
    println!("\n=== iter_map unresolved source ===");
    {
        let mut pipe = Pipeline::default();

        pipe.iter_map("loop", IterSource::map("missing"), |_, _, _| Ok(()))?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── Error: iter_map body references unknown variable ──
    println!("\n=== iter_map unresolved body reference ===");
    {
        let mut pipe = Pipeline::default();
        pipe.map("data")?.insert("k", "v")?;

        pipe.iter_map("loop", IterSource::map("data"), |_, _, body| {
            body.step::<SetVar>(
                "bad",
                params!(
                    "name" => "out",
                    "value" => Param::reference("phantom"),
                ),
            )?;
            Ok(())
        })?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    Ok(())
}
