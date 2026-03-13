use panopticon_core::{params, prelude::*};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Basic iter_array: iterate over items and capture each ──
    println!("=== Basic iter_array ===");
    {
        let mut pipe = Pipeline::default();
        pipe.array("numbers")?.push(10)?.push(20)?.push(30)?;

        pipe.iter_array(
            "process",
            IterSource::array("numbers"),
            |_index, item, body| {
                body.step::<SetVar>(
                    "capture",
                    params!(
                        "name" => "doubled",
                        "value" => Param::reference(item),
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

    // ── iter_array with parent variable references ──
    println!("\n=== iter_array referencing parent vars ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("prefix", "item_")?;
        pipe.array("items")?
            .push("alpha")?
            .push("beta")?
            .push("gamma")?;

        pipe.iter_array("loop", IterSource::array("items"), |_index, item, body| {
            body.step::<SetVar>(
                "combine",
                params!(
                    "name" => "label",
                    "value" => Param::template(vec![
                        Param::reference("prefix"),
                        Param::reference(item),
                    ]),
                ),
            )?;
            Ok(())
        })?;

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── iter_array with multiple body steps ──
    println!("\n=== iter_array with multi-step body ===");
    {
        let mut pipe = Pipeline::default();
        pipe.var("suffix", "_processed")?;
        pipe.array("words")?.push("hello")?.push("world")?;

        pipe.iter_array("each", IterSource::array("words"), |_index, item, body| {
            body.step::<SetVar>(
                "tag",
                params!(
                    "name" => "tagged",
                    "value" => Param::template(vec![
                        Param::reference(item),
                        Param::reference("suffix"),
                    ]),
                ),
            )?;
            body.step::<SetVar>(
                "wrap",
                params!(
                    "name" => "wrapped",
                    "value" => Param::template(vec![
                        Param::literal("["),
                        Param::reference("tagged"),
                        Param::literal("]"),
                    ]),
                ),
            )?;
            Ok(())
        })?;

        let complete = pipe.compile()?.run().wait()?;
        complete.debug();
    }

    // ── Error: iter_array with unresolved source ──
    println!("\n=== iter_array unresolved source ===");
    {
        let mut pipe = Pipeline::default();

        pipe.iter_array("loop", IterSource::array("nonexistent"), |_, _, _| Ok(()))?;

        match pipe.compile() {
            Err(e) => println!("  Caught: {}", e),
            Ok(_) => println!("  ERROR: should have failed!"),
        }
    }

    // ── Error: iter_array body references unknown variable ──
    println!("\n=== iter_array unresolved body reference ===");
    {
        let mut pipe = Pipeline::default();
        pipe.array("items")?.push(1)?;

        pipe.iter_array("loop", IterSource::array("items"), |_, _, body| {
            body.step::<SetVar>(
                "bad",
                params!(
                    "name" => "out",
                    "value" => Param::reference("ghost"),
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
