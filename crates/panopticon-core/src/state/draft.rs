use crate::execution::dependencies::DependencyTracker;
use crate::execution::node::prefix_node_names;
use crate::imports::*;

impl Pipeline<Draft> {
    /// Constructs an empty draft pipeline. Prefer [`Pipeline::default`] for
    /// the common case; this is the explicit form.
    pub fn new() -> Self {
        Pipeline {
            state: Draft {
                variables: Store::<StoreEntry>::new(),
                parameters: Store::<Parameters>::new(),
                returns: Store::<Parameters>::new(),
                execution_order: Vec::new(),
                registry: Registry::new(),
                hooks: Vec::new(),
                extensions: Extensions::new(),
            },
        }
    }
    fn validate_dependencies(&self) -> Result<(), DraftError> {
        DependencyTracker::validate(
            &self.state.variables,
            &self.state.execution_order,
            &self.state.parameters,
            &self.state.returns,
            &self.state.registry,
            &self.state.extensions,
        )
    }
    /// Runs draft-phase validation without consuming the pipeline. Simulates
    /// execution to catch unresolved references, forward references, and
    /// missing sources. Useful for inspecting the draft without committing
    /// to [`compile`](Self::compile).
    pub fn validate(&self) -> Result<(), DraftError> {
        self.validate_dependencies()
    }
    /// Promotes the pipeline from [`Draft`] to [`Ready`].
    ///
    /// Runs the same validation as [`validate`](Self::validate) — simulating
    /// execution to catch unresolved references, forward references, and
    /// missing sources — and on success consumes the draft and returns the
    /// ready pipeline. On failure the draft is lost.
    pub fn compile(self) -> Result<Pipeline<Ready>, DraftError> {
        self.validate_dependencies()?;
        Ok(Pipeline {
            state: Ready {
                variables: self.state.variables,
                parameters: self.state.parameters,
                returns: self.state.returns,
                execution_order: self.state.execution_order,
                registry: self.state.registry,
                hooks: self.state.hooks,
                extensions: self.state.extensions,
            },
        })
    }
    /// Defines a variable in the pipeline's variables store.
    ///
    /// The variable is visible to every subsequent step by the name given
    /// here. Returns [`DraftError::DuplicateVariable`] if the name is
    /// already in use.
    pub fn var<T: Into<String>, V: Into<Value>>(
        &mut self,
        name: T,
        value: V,
    ) -> Result<(), DraftError> {
        let name = name.into();

        self.state
            .variables
            .define_var(name, value)
            .map_err(|e| match e {
                StoreError::EntryAlreadyExists(name) => DraftError::DuplicateVariable { name },
                _ => unreachable!(),
            })?;
        Ok(())
    }
    /// Defines an array variable and returns an [`ArrayHandle`] for
    /// populating it with literal values at draft time.
    pub fn array<T: Into<String>>(
        &mut self,
        name: T,
    ) -> Result<ArrayHandle<'_, StoreEntry>, DraftError> {
        let name = name.into();
        self.state
            .variables
            .define_array(name)
            .map_err(|e| match e {
                StoreError::EntryAlreadyExists(name) => DraftError::DuplicateVariable { name },
                _ => unreachable!(),
            })
    }
    /// Defines a map variable and returns a [`MapHandle`] for populating it
    /// with literal entries at draft time.
    pub fn map<T: Into<String>>(
        &mut self,
        name: T,
    ) -> Result<MapHandle<'_, StoreEntry>, DraftError> {
        let name = name.into();
        self.state.variables.define_map(name).map_err(|e| match e {
            StoreError::EntryAlreadyExists(name) => DraftError::DuplicateVariable { name },
            _ => unreachable!(),
        })
    }
    /// Adds a step to the pipeline that will execute operation `O` with the
    /// given parameters.
    ///
    /// Registers `O` on the operation registry (idempotent) and records the
    /// step in the execution order. Use the [`params!`] macro to build the
    /// parameters map. Returns [`DraftError::DuplicateStep`] if the step
    /// name is already in use.
    pub fn step<O: Operation + 'static>(
        &mut self,
        name: impl Into<String>,
        params: Parameters,
    ) -> Result<(), DraftError> {
        let name = name.into();
        self.state.registry.register::<O>()?;
        self.state
            .parameters
            .insert(name.clone(), params)
            .map_err(|e| match e {
                StoreError::EntryAlreadyExists(name) => DraftError::DuplicateStep { name },
                _ => unreachable!(),
            })?;

        self.state.execution_order.push(ExecutionNode::Step {
            name,
            type_id: TypeId::of::<O>(),
        });
        Ok(())
    }
    /// Adds an iteration node that runs `body` once per element of an array
    /// source.
    ///
    /// The `body` closure receives the draft-time names of the `__index` and
    /// `__item` bindings injected into each iteration's cloned store, plus a
    /// child [`Pipeline<Draft>`] to populate. Each iteration executes on a
    /// clone of the parent store, so steps inside the body cannot see each
    /// other's global outputs across iterations.
    pub fn iter_array<F>(
        &mut self,
        name: impl Into<String>,
        source: IterSource,
        body: F,
    ) -> Result<(), DraftError>
    where
        F: FnOnce(
            /* index */ &str,
            /* item */ &str,
            &mut Pipeline<Draft>,
        ) -> Result<(), DraftError>,
    {
        let name = name.into();
        let index_binding = format!("{}.{}", name, ITER_INDEX);
        let item_binding = format!("{}.{}", name, ITER_ITEM);

        let mut child = Pipeline::new();
        body(&index_binding, &item_binding, &mut child)?;

        self.iter_array_from_child(name, source, child)
    }
    /// Splice a pre-built child `Pipeline<Draft>` into this pipeline as the
    /// body of an `iter_array` node.
    ///
    /// Equivalent to [`Pipeline::iter_array`] but accepts a child pipeline
    /// built imperatively (without a closure). Intended for callers that
    /// construct pipelines from external data (e.g. YAML loaders).
    ///
    /// Note: any hooks or extensions registered on the child pipeline are
    /// discarded. Only `parameters`, `execution_order`, and the operation
    /// registry are merged into the parent. Hooks and extensions must be
    /// registered on the root pipeline.
    pub fn iter_array_from_child(
        &mut self,
        name: impl Into<String>,
        source: IterSource,
        child: Pipeline<Draft>,
    ) -> Result<(), DraftError> {
        let name = name.into();
        let index_binding = format!("{}.{}", name, ITER_INDEX);
        let item_binding = format!("{}.{}", name, ITER_ITEM);

        let child_draft = child.state;

        for (step_key, params) in child_draft.parameters {
            let prefixed_key = format!("{}.{}", name, step_key);
            self.state
                .parameters
                .insert(prefixed_key, params)
                .map_err(|e| match e {
                    StoreError::EntryAlreadyExists(name) => DraftError::DuplicateStep { name },
                    _ => unreachable!(),
                })?;
        }

        self.state.registry.merge(child_draft.registry);

        let body_nodes = prefix_node_names(&name, child_draft.execution_order);

        self.state.execution_order.push(ExecutionNode::IterArray {
            name,
            source,
            index_binding,
            item_binding,
            body: body_nodes,
        });

        Ok(())
    }
    /// Adds an iteration node that runs `body` once per entry of a map source.
    ///
    /// The `body` closure receives the draft-time names of the `__key` and
    /// `__value` bindings injected into each iteration's cloned store, plus
    /// a child [`Pipeline<Draft>`] to populate. Each iteration executes on a
    /// clone of the parent store, so steps inside the body cannot see each
    /// other's global outputs across iterations.
    pub fn iter_map<F>(
        &mut self,
        name: impl Into<String>,
        source: IterSource,
        body: F,
    ) -> Result<(), DraftError>
    where
        F: FnOnce(
            /* key */ &str,
            /* value */ &str,
            &mut Pipeline<Draft>,
        ) -> Result<(), DraftError>,
    {
        let name = name.into();
        let key_binding = format!("{}.{}", name, ITER_KEY);
        let value_binding = format!("{}.{}", name, ITER_VALUE);

        let mut child = Pipeline::new();
        body(&key_binding, &value_binding, &mut child)?;

        self.iter_map_from_child(name, source, child)
    }
    /// Splice a pre-built child `Pipeline<Draft>` into this pipeline as the
    /// body of an `iter_map` node.
    ///
    /// Equivalent to [`Pipeline::iter_map`] but accepts a child pipeline built
    /// imperatively (without a closure). Intended for callers that construct
    /// pipelines from external data (e.g. YAML loaders).
    ///
    /// Note: any hooks or extensions registered on the child pipeline are
    /// discarded. Only `parameters`, `execution_order`, and the operation
    /// registry are merged into the parent. Hooks and extensions must be
    /// registered on the root pipeline.
    pub fn iter_map_from_child(
        &mut self,
        name: impl Into<String>,
        source: IterSource,
        child: Pipeline<Draft>,
    ) -> Result<(), DraftError> {
        let name = name.into();
        let key_binding = format!("{}.{}", name, ITER_KEY);
        let value_binding = format!("{}.{}", name, ITER_VALUE);

        let child_draft = child.state;

        for (step_key, params) in child_draft.parameters {
            let prefixed_key = format!("{}.{}", name, step_key);
            self.state
                .parameters
                .insert(prefixed_key, params)
                .map_err(|e| match e {
                    StoreError::EntryAlreadyExists(name) => DraftError::DuplicateStep { name },
                    _ => unreachable!(),
                })?;
        }

        self.state.registry.merge(child_draft.registry);

        let body_nodes = prefix_node_names(&name, child_draft.execution_order);

        self.state.execution_order.push(ExecutionNode::IterMap {
            name,
            source,
            key_binding,
            value_binding,
            body: body_nodes,
        });

        Ok(())
    }
    /// Adds a guard node that runs `body` only when the boolean referenced by
    /// `source` is true at runtime.
    ///
    /// Unlike [`iter_array`](Self::iter_array) and [`iter_map`](Self::iter_map),
    /// a guard body shares the parent scope — steps inside the guard see and
    /// contribute to the same global store as their siblings outside.
    pub fn guard<F>(
        &mut self,
        name: impl Into<String>,
        source: GuardSource,
        body: F,
    ) -> Result<(), DraftError>
    where
        F: FnOnce(&mut Pipeline<Draft>) -> Result<(), DraftError>,
    {
        let name = name.into();

        let mut child = Pipeline::new();
        body(&mut child)?;

        self.guard_from_child(name, source, child)
    }
    /// Splice a pre-built child `Pipeline<Draft>` into this pipeline as the
    /// body of a `guard` node.
    ///
    /// Equivalent to [`Pipeline::guard`] but accepts a child pipeline built
    /// imperatively (without a closure). Intended for callers that construct
    /// pipelines from external data (e.g. YAML loaders).
    ///
    /// Note: any hooks or extensions registered on the child pipeline are
    /// discarded. Only `parameters`, `execution_order`, and the operation
    /// registry are merged into the parent. Hooks and extensions must be
    /// registered on the root pipeline.
    pub fn guard_from_child(
        &mut self,
        name: impl Into<String>,
        source: GuardSource,
        child: Pipeline<Draft>,
    ) -> Result<(), DraftError> {
        let name = name.into();

        let child_draft = child.state;

        for (step_key, params) in child_draft.parameters {
            let prefixed_key = format!("{}.{}", name, step_key);
            self.state
                .parameters
                .insert(prefixed_key, params)
                .map_err(|e| match e {
                    StoreError::EntryAlreadyExists(name) => DraftError::DuplicateStep { name },
                    _ => unreachable!(),
                })?;
        }

        self.state.registry.merge(child_draft.registry);

        let body_nodes = prefix_node_names(&name, child_draft.execution_order);

        self.state.execution_order.push(ExecutionNode::Guard {
            name,
            source,
            body: body_nodes,
        });

        Ok(())
    }
    /// Declares a named return block that projects values out of the
    /// variables store at the end of execution.
    ///
    /// Each return block resolves its parameters against the final variables
    /// store after all steps finish; the resulting entries are accessible
    /// via `Pipeline<Complete>::returns`. Use a return block to expose
    /// exactly the slice of pipeline state a caller needs without leaking
    /// intermediate values.
    pub fn returns(
        &mut self,
        name: impl Into<String>,
        params: Parameters,
    ) -> Result<(), DraftError> {
        let name = name.into();
        self.state
            .returns
            .insert(name.clone(), params)
            .map_err(|e| match e {
                StoreError::EntryAlreadyExists(name) => DraftError::DuplicateReturn { name },
                _ => unreachable!(),
            })?;
        Ok(())
    }
    /// Attaches a [`Hook`] to the pipeline. Hooks observe or intercept
    /// execution events and fire in the order they were attached.
    pub fn hook(&mut self, hook: impl Into<Hook>) {
        self.state.hooks.push(hook.into());
    }
    /// Registers a named [`Extension`] — a shared, read-only service
    /// operations can look up by name via `Context::extension`.
    pub fn extension(&mut self, name: impl Into<String>, ext: impl Extension) {
        self.state.extensions.insert(name, ext);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_catches_unresolved_step_reference() {
        let mut pipe = Pipeline::default();
        pipe.var("number", 1).unwrap();

        pipe.step::<SetVar>(
            "bad_step",
            params!(
                "name" => "output",
                "value" => Param::reference("does_not_exist"),
            ),
        )
        .unwrap();

        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::UnresolvedReference {
                    step: "bad_step".into(),
                    reference: "does_not_exist".into(),
                }
            ),
            Ok(_) => panic!("expected compile to fail with UnresolvedReference"),
        }
    }

    #[test]
    fn compile_catches_unresolved_return_reference() {
        let mut pipe = Pipeline::default();
        pipe.var("number", 1).unwrap();

        pipe.step::<SetVar>(
            "ok_step",
            params!(
                "name" => "output",
                "value" => Param::reference("number"),
            ),
        )
        .unwrap();

        pipe.returns(
            "result",
            params!(
                "good" => Param::reference("output"),
                "bad" => Param::reference("never_created"),
            ),
        )
        .unwrap();

        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::UnresolvedReturn {
                    returns: "result".into(),
                    reference: "never_created".into(),
                }
            ),
            Ok(_) => panic!("expected compile to fail with UnresolvedReturn"),
        }
    }

    #[test]
    fn compile_catches_forward_reference() {
        let mut pipe = Pipeline::default();
        pipe.var("base", "start").unwrap();

        pipe.step::<SetVar>(
            "step_1",
            params!(
                "name" => "early",
                "value" => Param::reference("later_output"),
            ),
        )
        .unwrap();

        pipe.step::<SetVar>(
            "step_2",
            params!(
                "name" => "later_output",
                "value" => Param::reference("base"),
            ),
        )
        .unwrap();

        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::UnresolvedReference {
                    step: "step_1".into(),
                    reference: "later_output".into(),
                }
            ),
            Ok(_) => panic!("expected compile to fail with forward reference"),
        }
    }

    #[test]
    fn duplicate_variable_rejected() {
        let mut pipe = Pipeline::default();
        pipe.var("x", 1).unwrap();

        let err = pipe.var("x", 2).unwrap_err();
        assert_eq!(err, DraftError::DuplicateVariable { name: "x".into() });
    }

    #[test]
    fn duplicate_step_rejected() {
        let mut pipe = Pipeline::default();
        pipe.var("v", "val").unwrap();

        pipe.step::<SetVar>("same_name", params!("name" => "a", "value" => "b"))
            .unwrap();

        let err = pipe
            .step::<SetVar>("same_name", params!("name" => "c", "value" => "d"))
            .unwrap_err();
        assert_eq!(
            err,
            DraftError::DuplicateStep {
                name: "same_name".into()
            }
        );
    }

    // iter_array tests

    #[test]
    fn iter_array_compiles() {
        let mut pipe = Pipeline::default();
        pipe.array("items")
            .unwrap()
            .push(10)
            .unwrap()
            .push(20)
            .unwrap()
            .push(30)
            .unwrap();

        pipe.iter_array("loop", IterSource::array("items"), |_index, item, body| {
            body.step::<SetVar>(
                "set_current",
                params!(
                    "name" => "current",
                    "value" => Param::reference(item),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap();
    }

    #[test]
    fn iter_array_catches_unresolved_source() {
        let mut pipe = Pipeline::default();

        pipe.iter_array("loop", IterSource::array("nonexistent"), |_, _, _body| {
            Ok(())
        })
        .unwrap();

        assert!(pipe.compile().is_err());
    }

    #[test]
    fn iter_array_body_references_parent_vars() {
        let mut pipe = Pipeline::default();
        pipe.var("prefix", "item_").unwrap();
        pipe.array("items")
            .unwrap()
            .push("a")
            .unwrap()
            .push("b")
            .unwrap();

        pipe.iter_array("loop", IterSource::array("items"), |_index, item, body| {
            body.step::<SetVar>(
                "combine",
                params!(
                    "name" => "result",
                    "value" => Param::template(vec![
                        Param::reference("prefix"),
                        Param::reference(item),
                    ]),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap();
    }

    #[test]
    fn iter_array_catches_unresolved_body_reference() {
        let mut pipe = Pipeline::default();
        pipe.array("items").unwrap().push(1).unwrap();

        pipe.iter_array("loop", IterSource::array("items"), |_, _, body| {
            body.step::<SetVar>(
                "bad",
                params!(
                    "name" => "out",
                    "value" => Param::reference("ghost"),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        assert!(pipe.compile().is_err());
    }

    #[test]
    fn iter_array_runs_successfully() {
        let mut pipe = Pipeline::default();
        pipe.array("items")
            .unwrap()
            .push(10)
            .unwrap()
            .push(20)
            .unwrap();

        pipe.iter_array(
            "process",
            IterSource::array("items"),
            |_index, item, body| {
                body.step::<SetVar>(
                    "capture",
                    params!(
                        "name" => "result",
                        "value" => Param::reference(item),
                    ),
                )?;
                Ok(())
            },
        )
        .unwrap();

        // Should compile and run without error
        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn iter_array_body_cannot_see_other_iterations() {
        let mut pipe = Pipeline::default();
        pipe.array("items")
            .unwrap()
            .push("a")
            .unwrap()
            .push("b")
            .unwrap();

        pipe.iter_array("loop", IterSource::array("items"), |_index, item, body| {
            // Each iteration sets "output" — with pure scoping, iteration 1
            // should NOT see iteration 0's "output"
            body.step::<SetVar>(
                "set_it",
                params!(
                    "name" => "output",
                    "value" => Param::reference(item),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        // Should run without error — each iteration is independent
        pipe.compile().unwrap().run().wait().unwrap();
    }

    // iter_map tests

    #[test]
    fn iter_map_compiles() {
        let mut pipe = Pipeline::default();
        pipe.map("config")
            .unwrap()
            .insert("host", "localhost")
            .unwrap()
            .insert("port", "8080")
            .unwrap();

        pipe.iter_map("loop", IterSource::map("config"), |_key, value, body| {
            body.step::<SetVar>(
                "capture",
                params!(
                    "name" => "current",
                    "value" => Param::reference(value),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap();
    }

    #[test]
    fn iter_map_catches_unresolved_source() {
        let mut pipe = Pipeline::default();

        pipe.iter_map("loop", IterSource::map("nonexistent"), |_, _, _body| Ok(()))
            .unwrap();

        assert!(pipe.compile().is_err());
    }

    #[test]
    fn iter_map_body_references_parent_vars() {
        let mut pipe = Pipeline::default();
        pipe.var("prefix", "cfg_").unwrap();
        pipe.map("config")
            .unwrap()
            .insert("host", "localhost")
            .unwrap();

        pipe.iter_map("loop", IterSource::map("config"), |key, _value, body| {
            body.step::<SetVar>(
                "combine",
                params!(
                    "name" => "result",
                    "value" => Param::template(vec![
                        Param::reference("prefix"),
                        Param::reference(key),
                    ]),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap();
    }

    #[test]
    fn iter_map_catches_unresolved_body_reference() {
        let mut pipe = Pipeline::default();
        pipe.map("config").unwrap().insert("k", "v").unwrap();

        pipe.iter_map("loop", IterSource::map("config"), |_, _, body| {
            body.step::<SetVar>(
                "bad",
                params!(
                    "name" => "out",
                    "value" => Param::reference("ghost"),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        assert!(pipe.compile().is_err());
    }

    #[test]
    fn iter_map_runs_successfully() {
        let mut pipe = Pipeline::default();
        pipe.map("config")
            .unwrap()
            .insert("host", "localhost")
            .unwrap()
            .insert("port", "8080")
            .unwrap();

        pipe.iter_map("process", IterSource::map("config"), |_key, value, body| {
            body.step::<SetVar>(
                "capture",
                params!(
                    "name" => "result",
                    "value" => Param::reference(value),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn iter_map_body_cannot_see_other_iterations() {
        let mut pipe = Pipeline::default();
        pipe.map("data")
            .unwrap()
            .insert("a", "alpha")
            .unwrap()
            .insert("b", "beta")
            .unwrap();

        pipe.iter_map("loop", IterSource::map("data"), |_key, value, body| {
            body.step::<SetVar>(
                "set_it",
                params!(
                    "name" => "output",
                    "value" => Param::reference(value),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    // guard tests

    #[test]
    fn guard_compiles_with_valid_reference() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", true).unwrap();

        pipe.guard("check", GuardSource::boolean("flag"), |body| {
            body.step::<SetVar>(
                "inner",
                params!("name" => "result", "value" => "guarded"),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap();
    }

    #[test]
    fn guard_body_executes_when_true() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", true).unwrap();

        pipe.guard("check", GuardSource::boolean("flag"), |body| {
            body.step::<SetVar>(
                "inner",
                params!("name" => "result", "value" => "executed"),
            )?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn guard_body_skipped_when_false() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", false).unwrap();

        pipe.guard("check", GuardSource::boolean("flag"), |body| {
            body.step::<SetVar>(
                "inner",
                params!("name" => "result", "value" => "should_not_run"),
            )?;
            Ok(())
        })
        .unwrap();

        // Should complete without error even though body is skipped
        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn guard_output_available_downstream() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", true).unwrap();

        pipe.guard("check", GuardSource::boolean("flag"), |body| {
            body.step::<SetVar>(
                "produce",
                params!("name" => "guarded_value", "value" => "hello"),
            )?;
            Ok(())
        })
        .unwrap();

        // Step after guard references output produced inside guard body
        pipe.step::<SetVar>(
            "consume",
            params!(
                "name" => "final",
                "value" => Param::reference("guarded_value"),
            ),
        )
        .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn guard_catches_unresolved_boolean_reference() {
        let mut pipe = Pipeline::default();

        pipe.guard("check", GuardSource::boolean("nonexistent"), |_body| Ok(()))
            .unwrap();

        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::UnresolvedReference {
                    step: "check".into(),
                    reference: "nonexistent".into(),
                }
            ),
            Ok(_) => panic!("expected compile to fail"),
        }
    }

    #[test]
    fn guard_catches_unresolved_body_reference() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", true).unwrap();

        pipe.guard("check", GuardSource::boolean("flag"), |body| {
            body.step::<SetVar>(
                "bad",
                params!(
                    "name" => "out",
                    "value" => Param::reference("ghost"),
                ),
            )?;
            Ok(())
        })
        .unwrap();

        assert!(pipe.compile().is_err());
    }

    #[test]
    fn guard_runtime_error_for_non_boolean() {
        let mut pipe = Pipeline::default();
        pipe.var("not_a_bool", "hello").unwrap();

        pipe.guard("check", GuardSource::boolean("not_a_bool"), |body| {
            body.step::<SetVar>(
                "inner",
                params!("name" => "result", "value" => "nope"),
            )?;
            Ok(())
        })
        .unwrap();

        let result = pipe.compile().unwrap().run().wait();
        assert!(result.is_err());
    }

    // Extension validation tests
    //
    // These exercise the Draft -> Ready check that every `requires_extensions`
    // entry an operation declares is present on the pipeline. See
    // src/execution/dependencies.rs `validate_nodes` for the check itself.

    fn empty_params() -> Parameters {
        Parameters::new(std::collections::HashMap::new())
    }

    #[derive(Clone)]
    struct FakeDb;
    impl Extension for FakeDb {}

    #[derive(Clone)]
    struct FakeCache;
    impl Extension for FakeCache {}

    struct NeedsStaticDb;
    impl Operation for NeedsStaticDb {
        fn metadata() -> OperationMetadata {
            OperationMetadata {
                name: "NeedsStaticDb",
                description: "",
                inputs: &[],
                outputs: &[],
                requires_extensions: &[ExtensionSpec {
                    name: NameSpec::Static("db"),
                    description: "",
                    type_id: || TypeId::of::<FakeDb>(),
                }],
            }
        }
        fn execute(_: &mut Context) -> Result<(), OperationError> {
            Ok(())
        }
    }

    struct NeedsDerivedWithDefaultDb;
    impl Operation for NeedsDerivedWithDefaultDb {
        fn metadata() -> OperationMetadata {
            OperationMetadata {
                name: "NeedsDerivedWithDefaultDb",
                description: "",
                inputs: &[InputSpec {
                    name: "db_name",
                    ty: Type::Text,
                    required: false,
                    default: None,
                    description: "",
                }],
                outputs: &[],
                requires_extensions: &[ExtensionSpec {
                    name: NameSpec::DerivedWithDefault {
                        input_name: "db_name",
                        default: "default_db",
                    },
                    description: "",
                    type_id: || TypeId::of::<FakeDb>(),
                }],
            }
        }
        fn execute(_: &mut Context) -> Result<(), OperationError> {
            Ok(())
        }
    }

    struct NeedsDerivedFromDb;
    impl Operation for NeedsDerivedFromDb {
        fn metadata() -> OperationMetadata {
            OperationMetadata {
                name: "NeedsDerivedFromDb",
                description: "",
                inputs: &[InputSpec {
                    name: "db_name",
                    ty: Type::Text,
                    required: true,
                    default: None,
                    description: "",
                }],
                outputs: &[],
                requires_extensions: &[ExtensionSpec {
                    name: NameSpec::DerivedFrom("db_name"),
                    description: "",
                    type_id: || TypeId::of::<FakeDb>(),
                }],
            }
        }
        fn execute(_: &mut Context) -> Result<(), OperationError> {
            Ok(())
        }
    }

    #[test]
    fn static_extension_present_compiles() {
        let mut pipe = Pipeline::default();
        pipe.extension("db", FakeDb);
        pipe.step::<NeedsStaticDb>("step", empty_params()).unwrap();
        pipe.compile().unwrap();
    }

    #[test]
    fn static_extension_missing_fails() {
        let mut pipe = Pipeline::default();
        pipe.step::<NeedsStaticDb>("step", empty_params()).unwrap();
        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::MissingExtension {
                    step: "step".into(),
                    operation: "NeedsStaticDb",
                    extension: "db".into(),
                }
            ),
            Ok(_) => panic!("expected MissingExtension"),
        }
    }

    #[test]
    fn static_extension_wrong_type_fails() {
        let mut pipe = Pipeline::default();
        pipe.extension("db", FakeCache);
        pipe.step::<NeedsStaticDb>("step", empty_params()).unwrap();
        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::MissingExtension {
                    step: "step".into(),
                    operation: "NeedsStaticDb",
                    extension: "db".into(),
                }
            ),
            Ok(_) => panic!("expected MissingExtension"),
        }
    }

    #[test]
    fn derived_with_default_missing_default_fails() {
        let mut pipe = Pipeline::default();
        pipe.step::<NeedsDerivedWithDefaultDb>("step", empty_params())
            .unwrap();
        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::MissingExtension {
                    step: "step".into(),
                    operation: "NeedsDerivedWithDefaultDb",
                    extension: "default_db".into(),
                }
            ),
            Ok(_) => panic!("expected MissingExtension"),
        }
    }

    #[test]
    fn derived_with_default_present_compiles() {
        let mut pipe = Pipeline::default();
        pipe.extension("default_db", FakeDb);
        pipe.step::<NeedsDerivedWithDefaultDb>("step", empty_params())
            .unwrap();
        pipe.compile().unwrap();
    }

    #[test]
    fn derived_from_is_skipped_at_compile_time() {
        let mut pipe = Pipeline::default();
        pipe.var("chosen", "runtime_db").unwrap();
        pipe.step::<NeedsDerivedFromDb>(
            "step",
            params!("db_name" => Param::reference("chosen")),
        )
        .unwrap();
        // No extension registered — compile still succeeds because DerivedFrom
        // names can only be resolved at runtime.
        pipe.compile().unwrap();
    }

    #[test]
    fn static_extension_missing_inside_iter_body_fails() {
        let mut pipe = Pipeline::default();
        pipe.array("items").unwrap().push("a").unwrap();

        pipe.iter_array("loop", IterSource::array("items"), |_idx, _item, body| {
            body.step::<NeedsStaticDb>("inner", empty_params())?;
            Ok(())
        })
        .unwrap();

        match pipe.compile() {
            Err(e) => assert_eq!(
                e,
                DraftError::MissingExtension {
                    step: "loop.inner".into(),
                    operation: "NeedsStaticDb",
                    extension: "db".into(),
                }
            ),
            Ok(_) => panic!("expected MissingExtension"),
        }
    }

    #[test]
    fn malformed_extension_spec_derived_from_nonexistent_input_is_rejected() {
        struct BadOp;
        impl Operation for BadOp {
            fn metadata() -> OperationMetadata {
                OperationMetadata {
                    name: "BadOp",
                    description: "",
                    inputs: &[],
                    outputs: &[],
                    requires_extensions: &[ExtensionSpec {
                        name: NameSpec::DerivedFrom("nope"),
                        description: "",
                        type_id: || TypeId::of::<FakeDb>(),
                    }],
                }
            }
            fn execute(_: &mut Context) -> Result<(), OperationError> {
                Ok(())
            }
        }

        let mut pipe = Pipeline::default();
        let err = pipe.step::<BadOp>("bad", empty_params()).unwrap_err();
        match err {
            DraftError::InvalidMetadata { operation, .. } => assert_eq!(operation, "BadOp"),
            other => panic!("expected InvalidMetadata, got {other:?}"),
        }
    }

    #[test]
    fn guard_inside_iteration() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", true).unwrap();
        pipe.array("items").unwrap().push("a").unwrap().push("b").unwrap();

        pipe.iter_array("loop", IterSource::array("items"), |_index, item, body| {
            body.guard("check", GuardSource::boolean("flag"), |inner| {
                inner.step::<SetVar>(
                    "capture",
                    params!(
                        "name" => "captured",
                        "value" => Param::reference(item),
                    ),
                )?;
                Ok(())
            })?;
            Ok(())
        })
        .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    // _from_child tests — exercise the data-driven builder paths directly,
    // without going through the closure forms.

    #[test]
    fn iter_array_from_child_runs_successfully() {
        let mut pipe = Pipeline::default();
        pipe.array("items")
            .unwrap()
            .push(10)
            .unwrap()
            .push(20)
            .unwrap();

        let mut child = Pipeline::new();
        child
            .step::<SetVar>(
                "capture",
                params!(
                    "name" => "result",
                    "value" => Param::reference("loop.__item"),
                ),
            )
            .unwrap();

        pipe.iter_array_from_child("loop", IterSource::array("items"), child)
            .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn iter_map_from_child_runs_successfully() {
        let mut pipe = Pipeline::default();
        pipe.map("config")
            .unwrap()
            .insert("host", "localhost")
            .unwrap()
            .insert("port", "8080")
            .unwrap();

        let mut child = Pipeline::new();
        child
            .step::<SetVar>(
                "capture",
                params!(
                    "name" => "current",
                    "value" => Param::reference("loop.__value"),
                ),
            )
            .unwrap();

        pipe.iter_map_from_child("loop", IterSource::map("config"), child)
            .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn guard_from_child_runs_successfully() {
        let mut pipe = Pipeline::default();
        pipe.var("flag", true).unwrap();

        let mut child = Pipeline::new();
        child
            .step::<SetVar>(
                "produce",
                params!("name" => "guarded_value", "value" => "hello"),
            )
            .unwrap();

        pipe.guard_from_child("check", GuardSource::boolean("flag"), child)
            .unwrap();

        // Downstream step references the output produced inside the guard body,
        // proving guard scope-sharing semantics are preserved by the data-driven path.
        pipe.step::<SetVar>(
            "consume",
            params!(
                "name" => "final",
                "value" => Param::reference("guarded_value"),
            ),
        )
        .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }

    #[test]
    fn iter_array_from_child_catches_unresolved_body_reference() {
        let mut pipe = Pipeline::default();
        pipe.array("items").unwrap().push(1).unwrap();

        let mut child = Pipeline::new();
        child
            .step::<SetVar>(
                "bad",
                params!(
                    "name" => "out",
                    "value" => Param::reference("ghost"),
                ),
            )
            .unwrap();

        pipe.iter_array_from_child("loop", IterSource::array("items"), child)
            .unwrap();

        assert!(pipe.compile().is_err());
    }

    #[test]
    fn iter_array_from_child_empty_child_is_ok() {
        let mut pipe = Pipeline::default();
        pipe.array("items").unwrap().push(1).unwrap();

        let child = Pipeline::new();

        pipe.iter_array_from_child("loop", IterSource::array("items"), child)
            .unwrap();

        pipe.compile().unwrap().run().wait().unwrap();
    }
}
