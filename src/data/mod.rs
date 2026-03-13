/*
    TODO - Consider moving in all the store, value, collection related bits here.
*/
mod entry;
mod handle;
mod store;
mod value;

#[cfg(feature = "serde")]
mod de;

pub use entry::StoreEntry;
pub use handle::{ArrayHandle, MapHandle};
pub use store::Store;
pub use value::{Type, Value};

#[cfg(feature = "serde")]
pub use de::{DeserializeError, from_prefix_entries};

#[cfg(test)]
mod tests {
    use crate::imports::*;
    /*
        Wrote these to demo the Store API and verify the accessors work as expected. They also serve as a good reference for how to use the Store and its nested structures. The tests cover defining variables, arrays, maps, and nested combinations, as well as error handling for wrong accessors and missing entries.
    */

    #[test]
    fn define_flat_variables() {
        let mut store = Store::new();

        // From impls: &str -> Value, bool -> Value, i64 -> Value, f64 -> Value
        store.define_var("name", "Alice").unwrap();
        store.define_var("age", 30i64).unwrap();
        store.define_var("active", true).unwrap();
        store.define_var("score", 9.5f64).unwrap();
        store.define_var("empty", Value::Null).unwrap();

        // Store::get returns Result
        let name_entry = store.get("name").unwrap();
        println!("store.get(\"name\"): {:?}", name_entry);

        // get_value accessor — extracts &Value from a Var entry
        let name_val = name_entry.get_value().unwrap();
        println!("name_entry.get_value(): {:?}", name_val);
        assert_eq!(name_val, &Value::Text("Alice".into()));

        // as_var accessor — returns (&Value, &Type) tuple
        let (val, ty) = name_entry.as_var().unwrap();
        println!("name_entry.as_var(): val={:?}, ty={:?}", val, ty);
        assert_eq!(val, &Value::Text("Alice".into()));
        assert_eq!(ty, &Type::Text);

        // Verify all entries via get
        let age_entry = store.get("age").unwrap();
        println!("store.get(\"age\"): {:?}", age_entry);
        assert_eq!(age_entry.get_value().unwrap(), &Value::Integer(30));

        let active_entry = store.get("active").unwrap();
        println!("store.get(\"active\"): {:?}", active_entry);
        assert_eq!(active_entry.get_value().unwrap(), &Value::Boolean(true));

        let score_entry = store.get("score").unwrap();
        println!("store.get(\"score\"): {:?}", score_entry);
        assert_eq!(score_entry.get_value().unwrap(), &Value::Float(9.5));

        let empty_entry = store.get("empty").unwrap();
        println!("store.get(\"empty\"): {:?}", empty_entry);
        assert_eq!(empty_entry.get_value().unwrap(), &Value::Null);

        // AccessError on wrong type — calling as_array on a Var
        let arr_err = name_entry.as_array();
        println!("name_entry.as_array(): {:?}", arr_err);
        assert_eq!(arr_err, Err(AccessError::NotAnArray("Var")));

        // Missing entry returns Err
        let missing = store.get("nonexistent");
        println!("store.get(\"nonexistent\"): {:?}", missing);
        assert_eq!(
            missing,
            Err(StoreError::EntryNotFound("nonexistent".into()))
        );

        // Duplicate name should fail
        let err = store.define_var("name", "Bob");
        println!("duplicate define_var: {:?}", err);
        assert!(err.is_err());
    }

    #[test]
    fn define_flat_array() {
        let mut store = Store::new();

        let mut arr = store.define_array("numbers").unwrap();
        arr.push(1).unwrap();
        arr.push(2).unwrap();
        arr.push(3).unwrap();

        let entry = store.get("numbers").unwrap();
        println!("store.get(\"numbers\"): {:?}", entry);

        // as_array accessor
        let items = entry.as_array().unwrap();
        println!("entry.as_array(): len={}", items.len());
        assert_eq!(items.len(), 3);

        // get_index accessor
        let first = entry.get_index(0).unwrap();
        println!("entry.get_index(0): {:?}", first);
        assert_eq!(first.get_value().unwrap(), &Value::Integer(1));

        let third = entry.get_index(2).unwrap();
        println!("entry.get_index(2): {:?}", third);
        assert_eq!(third.get_value().unwrap(), &Value::Integer(3));

        // Out of bounds
        let oob = entry.get_index(10);
        println!("entry.get_index(10): {:?}", oob);
        assert_eq!(oob, Err(AccessError::IndexOutOfBounds(10)));

        // as_map on an array should fail
        let map_err = entry.as_map();
        println!("entry.as_map(): {:?}", map_err);
        assert_eq!(map_err, Err(AccessError::NotAMap("Array")));
    }

    #[test]
    fn define_flat_map() {
        let mut store = Store::new();

        let mut map = store.define_map("person").unwrap();
        map.insert("name", "Alice").unwrap();
        map.insert("age", 30).unwrap();

        let entry = store.get("person").unwrap();
        println!("store.get(\"person\"): {:?}", entry);

        // as_map accessor
        let map_data = entry.as_map().unwrap();
        println!("entry.as_map(): len={}", map_data.len());
        assert_eq!(map_data.len(), 2);

        // get_key accessor
        let name_entry = entry.get_key("name").unwrap();
        println!("entry.get_key(\"name\"): {:?}", name_entry);
        assert_eq!(
            name_entry.get_value().unwrap(),
            &Value::Text("Alice".into())
        );

        let age_entry = entry.get_key("age").unwrap();
        println!("entry.get_key(\"age\"): {:?}", age_entry);
        assert_eq!(age_entry.get_value().unwrap(), &Value::Integer(30));

        // Missing key
        let missing = entry.get_key("missing");
        println!("entry.get_key(\"missing\"): {:?}", missing);
        assert_eq!(missing, Err(AccessError::NotFound("missing".into())));

        // get_index on a map should fail
        let idx_err = entry.get_index(0);
        println!("entry.get_index(0): {:?}", idx_err);
        assert_eq!(idx_err, Err(AccessError::NotAnArray("Map")));
    }

    #[test]
    fn define_array_of_maps() {
        let mut store = Store::new();

        let mut arr = store.define_array("people").unwrap();

        {
            let mut person1 = arr.push_map().unwrap();
            person1.insert("name", "Alice").unwrap();
            person1.insert("age", 30).unwrap();
        }

        let mut person2 = arr.push_map().unwrap();
        person2.insert("name", "Bob").unwrap();
        person2.insert("age", 25).unwrap();

        let entry = store.get("people").unwrap();
        println!("store.get(\"people\"): {:?}", entry);

        // Chained accessors: get_index -> get_key -> get_value
        let alice_name = entry
            .get_index(0)
            .unwrap()
            .get_key("name")
            .unwrap()
            .get_value()
            .unwrap();
        println!("people[0].name: {:?}", alice_name);
        assert_eq!(alice_name, &Value::Text("Alice".into()));

        let bob_name = entry
            .get_index(1)
            .unwrap()
            .get_key("name")
            .unwrap()
            .get_value()
            .unwrap();
        println!("people[1].name: {:?}", bob_name);
        assert_eq!(bob_name, &Value::Text("Bob".into()));

        let bob_age = entry
            .get_index(1)
            .unwrap()
            .get_key("age")
            .unwrap()
            .get_value()
            .unwrap();
        println!("people[1].age: {:?}", bob_age);
        assert_eq!(bob_age, &Value::Integer(25));

        // Verify structure via as_* accessors
        let items = entry.as_array().unwrap();
        assert_eq!(items.len(), 2);
        let first_map = items[0].as_map().unwrap();
        println!(
            "people[0] as_map keys: {:?}",
            first_map.keys().collect::<Vec<_>>()
        );
        assert_eq!(first_map.len(), 2);
    }

    #[test]
    fn define_map_of_arrays() {
        let mut store = Store::new();

        let mut map = store.define_map("data").unwrap();

        let mut scores = map.insert_array("scores").unwrap();
        scores.push(95).unwrap();
        scores.push(87).unwrap();
        scores.push(92).unwrap();

        let mut tags = map.insert_array("tags").unwrap();
        tags.push("rust").unwrap();
        tags.push("test").unwrap();

        let entry = store.get("data").unwrap();
        println!("store.get(\"data\"): {:?}", entry);

        // Chained accessors: get_key -> get_index -> get_value
        let first_score = entry
            .get_key("scores")
            .unwrap()
            .get_index(0)
            .unwrap()
            .get_value()
            .unwrap();
        println!("data.scores[0]: {:?}", first_score);
        assert_eq!(first_score, &Value::Integer(95));

        let second_tag = entry
            .get_key("tags")
            .unwrap()
            .get_index(1)
            .unwrap()
            .get_value()
            .unwrap();
        println!("data.tags[1]: {:?}", second_tag);
        assert_eq!(second_tag, &Value::Text("test".into()));

        // Verify structure
        let scores_arr = entry.get_key("scores").unwrap().as_array().unwrap();
        println!("data.scores len: {}", scores_arr.len());
        assert_eq!(scores_arr.len(), 3);

        let tags_arr = entry.get_key("tags").unwrap().as_array().unwrap();
        println!("data.tags len: {}", tags_arr.len());
        assert_eq!(tags_arr.len(), 2);

        // Wrong accessor type on nested entry
        let not_a_map = entry.get_key("scores").unwrap().as_map();
        println!("data.scores.as_map(): {:?}", not_a_map);
        assert_eq!(not_a_map, Err(AccessError::NotAMap("Array")));
    }
}
