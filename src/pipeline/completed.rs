use super::{Completed, Draft, Ready};
use crate::imports::*;

impl Pipeline<Completed> {
    pub async fn results(
        &self,
        ResultSettings {
            output_path,
            format,
            excluded_commands,
        }: ResultSettings,
    ) -> Result<ResultStore> {
        use super::results::write_tabular;

        let context = &self.state.context;

        std::fs::create_dir_all(&output_path)
            .with_context(|| format!("Failed to create output directory: {:?}", output_path))?;

        let mut results = Vec::new();

        for cmd in &self.commands {
            let namespace = self
                .namespaces
                .get(cmd.namespace_index)
                .context("Invalid namespace index on command")?;

            let base_path = StorePath::from_segments([namespace.name(), cmd.name.as_str()]);

            // Skip excluded commands
            if excluded_commands
                .iter()
                .any(|ex| base_path.starts_with(ex) || ex == &base_path)
            {
                continue;
            }

            // Determine which StorePaths to collect from based on namespace type
            let source_paths: Vec<StorePath> = match namespace.ty() {
                ExecutionMode::Once | ExecutionMode::Static { .. } => {
                    vec![base_path]
                }
                ExecutionMode::Iterative { .. } => {
                    // Probe indices until we find one with no "status" metadata
                    // (every executed command produces a "status" result via COMMON_RESULTS)
                    let mut paths = Vec::new();
                    let mut index = 0;
                    loop {
                        let indexed_path = base_path.with_index(index);
                        let status_path = indexed_path.with_segment("status");
                        if context.scalar().get(&status_path).await?.is_some() {
                            paths.push(indexed_path);
                            index += 1;
                        } else {
                            break;
                        }
                    }
                    paths
                }
            };

            for source in source_paths {
                let mut meta = HashMap::new();
                let mut data = HashMap::new();

                for result_spec in &cmd.expected_results {
                    // Resolve one or more (field_name, kind, type_def) entries from the spec.
                    // DerivedFromSingleAttribute over an array attribute expands to one entry
                    // per element, keyed by the element's name_field value.
                    let entries: Vec<(String, &ResultKind, Option<&TypeDef<String>>)> =
                        match result_spec {
                            ResultSpec::Field { name, kind, ty, .. } => {
                                vec![(name.clone(), kind, Some(ty))]
                            }
                            ResultSpec::DerivedFromSingleAttribute {
                                name_field,
                                kind,
                                ty,
                                attribute,
                                ..
                            } => {
                                let attr_value = cmd.attributes.get(attribute.as_str());
                                match attr_value {
                                    Some(ScalarValue::Array(arr)) => arr
                                        .iter()
                                        .filter_map(|elem| {
                                            elem.as_object()
                                                .and_then(|obj| obj.get(name_field.name()))
                                                .and_then(|v| v.as_str())
                                                .map(|s| (s.to_string(), kind, ty.as_ref()))
                                        })
                                        .collect(),
                                    Some(v) => {
                                        let resolved = v
                                            .as_str()
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| name_field.name().clone());
                                        vec![(resolved, kind, ty.as_ref())]
                                    }
                                    None => {
                                        vec![(name_field.name().clone(), kind, ty.as_ref())]
                                    }
                                }
                            }
                        };

                    for (field_name, kind, type_def) in entries {
                        let field_path = source.with_segment(&field_name);
                        let is_tabular = type_def.is_some_and(|td| matches!(td, TypeDef::Tabular));

                        if is_tabular {
                            if let Some(df) = context.tabular().get(&field_path).await? {
                                let rows_count = df.height();
                                let columns_count = df.width();

                                let file_name =
                                    format!("{}.{}", field_path.to_dotted(), format.extension());
                                let file_path = output_path.join(&file_name);
                                write_tabular(&df, &file_path, &format)?;

                                data.insert(
                                    field_path,
                                    ResultValue::Tabular {
                                        path: file_path,
                                        format: format.clone(),
                                        rows_count,
                                        columns_count,
                                    },
                                );
                            }
                        } else if let Some(value) = context.scalar().get(&field_path).await? {
                            match kind {
                                ResultKind::Meta => {
                                    meta.insert(field_path, value);
                                }
                                ResultKind::Data => {
                                    let ty = type_def
                                        .and_then(|td| match td {
                                            TypeDef::Scalar(st) => Some(st.clone()),
                                            _ => None,
                                        })
                                        .unwrap_or_else(|| scalar_type_of(&value));

                                    data.insert(field_path, ResultValue::Scalar { ty, value });
                                }
                            }
                        }
                    }
                }

                results.push(CommandResults { source, meta, data });
            }
        }

        Ok(ResultStore { results })
    }

    pub fn restart(self) -> Pipeline<Ready> {
        Pipeline::<Ready> {
            namespaces: self.namespaces,
            commands: self.commands,
            state: Ready,
        }
    }

    pub fn edit(self) -> Pipeline<Draft> {
        Pipeline::<Draft> {
            namespaces: self.namespaces,
            commands: self.commands,
            state: Draft,
        }
    }

    /*
        Not going to use this but just putting it here for reference, love this "argument destruction" pattern
        Aware/for reference Axum does alot of this kind of stuff in it's API.

        pub fn edit(
            Pipeline {
                namespaces,
                commands,
                ..
            }: Self,
        ) -> Pipeline<Draft> {
            Pipeline::<Draft> {
                namespaces,
                commands,
                state: Draft,
            }
        }
    */
}
