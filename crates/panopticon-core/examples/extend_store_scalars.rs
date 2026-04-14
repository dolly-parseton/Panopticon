//! Defining scalar variables in a Store.
//!
//! The Store is the data backing a Pipeline. Operations read from and write to
//! it. This example shows how to define scalar variables and read them back,
//! using the ergonomic From/Into conversions that let you pass Rust primitives
//! directly — no `.into()` calls required.

use panopticon_core::extend::{Store, Type, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::new();

    // define_var accepts any V: Into<Value>.
    // From impls exist for &str, String, bool, i64, f64, so you can pass
    // these directly without calling .into().
    store.define_var("name", "Alice")?;
    store.define_var("age", 30i64)?;
    store.define_var("active", true)?;
    store.define_var("score", 9.5f64)?;
    store.define_var("empty", Value::Null)?;

    // Reading back: Store::get returns &StoreEntry.
    // get_value() extracts the &Value from a Var entry.
    let name = store.get("name")?.get_value()?;
    println!("name = {name:?}");

    // as_var() returns (&Value, &Type) — useful when you need to inspect both.
    let (val, ty) = store.get("age")?.as_var()?;
    println!("age = {val:?} (type: {ty:?})");

    // Type is inferred from the value at define time.
    assert_eq!(store.get("active")?.as_var()?.1, &Type::Boolean);
    assert_eq!(store.get("score")?.as_var()?.1, &Type::Float);
    assert_eq!(store.get("empty")?.as_var()?.1, &Type::Null);

    // Duplicate names are rejected with StoreError::EntryAlreadyExists.
    let err = store.define_var("name", "Bob");
    println!("duplicate define_var: {err:?}");
    assert!(err.is_err());

    println!("store_scalars: ok");
    Ok(())
}
