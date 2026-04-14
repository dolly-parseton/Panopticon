//! Deserializing a pipeline's returns into a typed struct.
//!
//! `CompletePipeline::deserialize_returns` pulls a named return group through
//! serde into any `Deserialize` target, so downstream code can work with
//! domain types instead of poking at `Value` trees. Requires the `serde`
//! feature.

use panopticon_core::{params, prelude::*};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Summary {
    greeting: String,
    farewell: String,
    count: i64,
    tags: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pipe = Pipeline::default();

    pipe.var("name", "world")?;

    pipe.step::<SetVar>(
        "make_greeting",
        params!(
            "name" => "greeting",
            "value" => Param::template(vec![
                Param::literal("hello, "),
                Param::reference("name"),
            ]),
        ),
    )?;

    pipe.step::<SetVar>(
        "make_farewell",
        params!(
            "name" => "farewell",
            "value" => Param::template(vec![
                Param::literal("goodbye, "),
                Param::reference("name"),
            ]),
        ),
    )?;

    pipe.step::<SetVar>("make_count", params!("name" => "count", "value" => 42i64))?;

    pipe.returns(
        "output",
        params!(
            "greeting" => Param::reference("greeting"),
            "farewell" => Param::reference("farewell"),
            "count" => Param::reference("count"),
            "tags" => Param::array(vec![
                Param::literal("demo"),
                Param::literal("1"),
            ]),
        ),
    )?;

    let complete = pipe.compile()?.run().wait()?;

    let summary: Summary = complete.deserialize_returns("output")?;
    println!("{:#?}", summary);

    Ok(())
}
