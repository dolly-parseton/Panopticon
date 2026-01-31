use crate::imports::*;

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "tera.pest"] // relative to src directory
struct TeraParser;

#[tracing::instrument(
    name = "parse_template_dependencies",
    level = "debug",
    skip_all,
    fields(template_hash, initial_count, final_count)
)]
pub fn parse_template_dependencies(
    template: &str,
    dependencies: &mut HashSet<StorePath>,
) -> Result<()> {
    if tracing::enabled!(tracing::Level::DEBUG) {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        template.hash(&mut hasher);
        let template_hash = format!("{:x}", hasher.finish());
        tracing::Span::current().record("template_hash", template_hash.as_str());
        tracing::Span::current().record("initial_count", dependencies.len());
    }

    let parse_result = TeraParser::parse(Rule::template, template);
    let pairs = match parse_result {
        Ok(pairs) => pairs,
        Err(e) => {
            return Err(anyhow::anyhow!("Template parsing error: {}", e));
        }
    };
    extract_identifiers_recursive(pairs, dependencies);

    if tracing::enabled!(tracing::Level::DEBUG) {
        tracing::Span::current().record("final_count", dependencies.len());
    }

    Ok(())
}

fn extract_identifiers_recursive<'a>(
    pairs: impl Iterator<Item = Pair<'a, Rule>>,
    vars: &mut HashSet<StorePath>,
) {
    for pair in pairs {
        if matches!(pair.as_rule(), Rule::dotted_square_bracket_ident) {
            vars.insert(StorePath::from_dotted(pair.as_str()));
        }
        extract_identifiers_recursive(pair.into_inner(), vars);
    }
}

// TODO - Expand test coverage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_context_dependencies() {
        let template = r#"
            Hello, {{ name }}!
            Your age is {{ age }}.
            {% if is_member %}
                Welcome back, member!
            {% endif %}
        "#;

        let mut vars = HashSet::new();
        parse_template_dependencies(template, &mut vars).unwrap();
        println!("Extracted variables: {:?}", vars);
        let expected_vars: HashSet<StorePath> = [
            StorePath::from_dotted("name"),
            StorePath::from_dotted("age"),
            StorePath::from_dotted("is_member"),
        ]
        .into_iter()
        .collect();
        assert_eq!(vars, expected_vars);
    }
}
