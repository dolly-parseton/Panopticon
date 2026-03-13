//! Arrays and maps — chaining, closures, and nested construction.
//!
//! ArrayHandle::push and MapHandle::insert return Result<&mut Self>, enabling
//! fluent chaining with `?`. The Store also provides with_array/with_map
//! closure methods that scope the handle lifetime automatically.

use panopticon_core::extend::{Store, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::new();

    // --- Chainable push ---
    // push() accepts V: Into<StoreEntry>. The blanket impl
    // `From<T: Into<Value>> for StoreEntry` means primitives work directly.
    let mut arr = store.define_array("numbers")?.push(1)?.push(2)?.push(3)?;

    let items = store.get("numbers")?.as_array()?;
    println!("numbers: len={}", items.len());
    assert_eq!(items.len(), 3);

    // --- Chainable insert ---
    // insert() accepts K: Into<String> and V: Into<StoreEntry>.
    let mut map = store.define_map("config")?;
    map.insert("host", "localhost")?
        .insert("port", 8080i64)?
        .insert("debug", true)?;

    let host = store.get("config")?.get_key("host")?.get_value()?;
    println!("config.host = {host:?}");

    // --- Closure API on Store ---
    // with_array scopes the handle so you don't need an explicit drop.
    store.with_array("tags", |arr| {
        arr.push("rust")?.push("pipeline")?.push("store")?;
        Ok(())
    })?;

    let tags = store.get("tags")?.as_array()?;
    println!("tags: len={}", tags.len());

    store.with_map("metadata", |map| {
        map.insert("version", "0.1.0")?.insert("stable", false)?;
        Ok(())
    })?;

    let version = store.get("metadata")?.get_key("version")?.get_value()?;
    println!("metadata.version = {version:?}");

    // --- Nested construction via MapHandle closures ---
    // with_array and with_map on MapHandle let you build nested structures
    // without manually managing sub-handles.
    store.with_map("server", |map| {
        map.insert("name", "prod-01")?;

        map.with_array("ports", |arr| {
            arr.push(80)?.push(443)?;
            Ok(())
        })?;

        map.with_map("tls", |tls| {
            tls.insert("enabled", true)?
                .insert("cert_path", "/etc/ssl/cert.pem")?;
            Ok(())
        })?;

        Ok(())
    })?;

    let tls_enabled = store
        .get("server")?
        .get_key("tls")?
        .get_key("enabled")?
        .get_value()?;
    println!("server.tls.enabled = {tls_enabled:?}");

    // --- push_map on ArrayHandle ---
    // Builds an array of maps — the most common pattern for tabular results.
    let mut arr = store.define_array("users")?;
    {
        let mut user = arr.push_map()?;
        user.insert("name", "Alice")?.insert("role", "admin")?;
    }
    {
        let mut user = arr.push_map()?;
        user.insert("name", "Bob")?.insert("role", "viewer")?;
    }

    let bob_role = store
        .get("users")?
        .get_index(1)?
        .get_key("role")?
        .get_value()?;
    println!("users[1].role = {bob_role:?}");
    assert_eq!(bob_role, &Value::Text("viewer".into()));

    println!("store_collections: ok");
    Ok(())
}
