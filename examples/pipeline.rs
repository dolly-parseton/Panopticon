use panopticon_core::{extend::Type, params, prelude::*};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Happy path: derived outputs resolved at compile time ──
    println!("=== Valid pipeline ===");
    {
        let mut pipe = Pipeline::default();

        pipe.var("number", 1)?;
        pipe.var("string", "example")?;
        pipe.array("array")?.push(1)?.push(2)?.push(3)?;

        // Derived output name from a variable ref: "string" → "example"
        pipe.step::<SetVar>(
            "set_var_1",
            params!(
                "name" => Param::reference("string"),
                "value" => Param::reference("number"),
            ),
        )?;

        // Derived output name from a literal: "greeting"
        pipe.step::<SetVar>(
            "set_var_2",
            params!(
                "name" => "greeting",
                "value" => "hello world",
            ),
        )?;

        // Chained: use derived output "greeting" from set_var_2 as a variable reference
        pipe.step::<SetVar>(
            "set_var_3",
            params!(
                "name" => "farewell",
                "value" => Param::reference("greeting"),
            ),
        )?;

        // Template referencing a derived output
        pipe.step::<SetVar>(
            "set_var_4",
            params!(
                "name" => "message",
                "value" => Param::template(vec![
                    Param::reference("greeting"),
                    Param::literal(" and goodbye"),
                ]),
            ),
        )?;

        pipe.returns(
            "summary",
            params!(
                "original_number" => Param::reference("number"),
                "original_string" => Param::reference("string"),
                "the_array" => Param::reference("array"),
                "greeting" => Param::reference("greeting"),
                "derived_from_ref" => Param::reference("example"),
                "farewell" => Param::reference("farewell"),
                "message" => Param::reference("message"),
            ),
        )?;

        // Logger: writes each event to stdout
        pipe.hook(Logger::new().writer(std::io::stdout()));

        // Profiler: tracks per-step wall-clock duration
        let profiler = Profiler::new().writer(std::io::stdout());
        let timings = profiler.timings();
        pipe.hook(profiler);

        // EventLog: collects structured event records for post-mortem inspection
        let event_log = EventLog::new();
        let log = event_log.log();
        pipe.hook(event_log);

        // Timeout: aborts if pipeline exceeds 5 seconds
        pipe.hook(Timeout::new(Duration::from_secs(5)));

        // StoreValidator: assert "greeting" exists as Text after set_var_2
        pipe.hook(
            StoreValidator::new()
                .after_step("set_var_2")
                .expect("greeting", Type::Text),
        );

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();

        // Read profiler timings programmatically
        println!("\nProgrammatic timings:");
        for (name, dur) in timings.lock().unwrap().iter() {
            println!("  {} took {:.3}ms", name, dur.as_secs_f64() * 1000.0);
        }

        // Read event log
        println!("\nEvent log ({} events):", log.lock().unwrap().len());
        for record in log.lock().unwrap().iter() {
            println!(
                "  [{:.3}ms] {:?}{}",
                record.elapsed.as_secs_f64() * 1000.0,
                record.kind,
                record
                    .step_name
                    .as_deref()
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default(),
            );
        }
    }

    // ── Error: step references a variable that doesn't exist ──
    println!("\n=== Unresolved step reference ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("number", 1)?;

        pipe.step::<SetVar>(
            "bad_step",
            params!(
                "name" => "output",
                "value" => Param::reference("does_not_exist"),
            ),
        )?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── Error: return references something that was never produced ──
    println!("\n=== Unresolved return reference ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("number", 1)?;

        pipe.step::<SetVar>(
            "ok_step",
            params!(
                "name" => "output",
                "value" => Param::reference("number"),
            ),
        )?;

        pipe.returns(
            "result",
            params!(
                "good" => Param::reference("output"),
                "bad" => Param::reference("never_created"),
            ),
        )?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── Error: step references a later step's output (ordering) ──
    println!("\n=== Forward reference (wrong order) ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("base", "start")?;

        // This step tries to reference "later_output" which doesn't exist yet
        pipe.step::<SetVar>(
            "step_1",
            params!(
                "name" => "early",
                "value" => Param::reference("later_output"),
            ),
        )?;

        pipe.step::<SetVar>(
            "step_2",
            params!(
                "name" => "later_output",
                "value" => Param::reference("base"),
            ),
        )?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── Error: duplicate variable name ──
    println!("\n=== Duplicate variable ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("x", 1)?;
        match pipe.var("x", 2) {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── Error: duplicate step name ──
    println!("\n=== Duplicate step ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("v", "val")?;

        pipe.step::<SetVar>("same_name", params!("name" => "a", "value" => "b"))?;
        match pipe.step::<SetVar>("same_name", params!("name" => "c", "value" => "d")) {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── StepFilter: deny a step from executing ──
    println!("\n=== StepFilter deny ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("x", 1)?;

        pipe.step::<SetVar>("allowed_step", params!("name" => "a", "value" => "ok"))?;
        pipe.step::<SetVar>(
            "blocked_step",
            params!("name" => "b", "value" => Param::reference("a")),
        )?;

        pipe.returns("result", params!("a" => Param::reference("a")))?;

        pipe.hook(StepFilter::deny(["blocked_step"]));

        match pipe.compile()?.run().wait() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have been denied!"),
        }
    }

    Ok(())
}
