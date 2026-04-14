//! Conditional execution with `guard` blocks.
//!
//! A guard runs its body only when a boolean `GuardSource` resolves to true.
//! The guard reference is checked at compile time, so an unresolved source
//! fails `compile()` rather than the run. Outputs produced inside the body
//! are visible to later steps in the parent pipeline.

use panopticon_core::{params, prelude::*};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Basic guard: Compare produces true, body executes ──
    println!("=== Guard (true) ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("status_code", 200i64)?;

        pipe.step::<Compare>(
            "is_ok",
            params!(
                "left" => Param::reference("status_code"),
                "op" => "eq",
                "right" => 200i64,
                "name" => "should_run",
            ),
        )?;

        pipe.guard("check", GuardSource::boolean("is_ok.should_run"), |body| {
            body.step::<SetVar>(
                "produce",
                params!("name" => "result", "value" => "guard body executed"),
            )?;
            Ok(())
        })?;

        pipe.hook(Logger::new().writer(std::io::stdout()));
        pipe.hook(Timeout::new(Duration::from_secs(5)));

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── Guard skipped: Compare produces false ──
    println!("\n=== Guard (false) ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("status_code", 404i64)?;

        pipe.step::<Compare>(
            "is_ok",
            params!(
                "left" => Param::reference("status_code"),
                "op" => "eq",
                "right" => 200i64,
                "name" => "should_run",
            ),
        )?;

        pipe.guard("check", GuardSource::boolean("is_ok.should_run"), |body| {
            body.step::<SetVar>(
                "produce",
                params!("name" => "result", "value" => "should not appear"),
            )?;
            Ok(())
        })?;

        pipe.hook(Logger::new().writer(std::io::stdout()));

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── Guard output used by downstream step ──
    println!("\n=== Guard output used downstream ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("count", 5i64)?;

        pipe.step::<Compare>(
            "has_results",
            params!(
                "left" => Param::reference("count"),
                "op" => "gt",
                "right" => 0i64,
                "name" => "flag",
            ),
        )?;

        pipe.guard("gate", GuardSource::boolean("has_results.flag"), |body| {
            body.step::<SetVar>(
                "create",
                params!("name" => "message", "value" => "from inside guard"),
            )?;
            Ok(())
        })?;

        pipe.step::<SetVar>(
            "consume",
            params!(
                "name" => "final_output",
                "value" => Param::template(vec![
                    Param::literal("received: "),
                    Param::reference("message"),
                ]),
            ),
        )?;

        pipe.hook(Logger::new().writer(std::io::stdout()));

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── Error: unresolved guard reference ──
    println!("\n=== Guard unresolved reference ===");
    {
        let mut pipe = Pipeline::default();

        pipe.guard("check", GuardSource::boolean("nonexistent"), |_body| Ok(()))?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    Ok(())
}
