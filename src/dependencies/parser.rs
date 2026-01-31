use crate::imports::*;

use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "tera.pest"] // relative to src directory
struct TeraParser;

pub fn parse_template_dependencies(
    template: &str,
    dependencies: &mut HashSet<StorePath>,
) -> Result<()> {
    let parse_result = TeraParser::parse(Rule::template, template);
    let pairs = match parse_result {
        Ok(pairs) => pairs,
        Err(e) => {
            return Err(anyhow::anyhow!("Template parsing error: {}", e));
        }
    };
    extract_identifiers_recursive(pairs, dependencies);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_identifiers() {
        let template = r#"
        Hello {{ user.name }}!
        Your order {{ order.id }} is confirmed.
        {% for item in order.items %}
            - {{ item.name }}: {{ item.price }}
        {% endfor %}
        "#;

        let mut dependencies = HashSet::new();
        parse_template_dependencies(template, &mut dependencies).unwrap();

        let expected: HashSet<StorePath> = vec![
            StorePath::from_dotted("user.name"),
            StorePath::from_dotted("order.id"),
            StorePath::from_dotted("order.items"),
            StorePath::from_dotted("item.name"),
            StorePath::from_dotted("item.price"),
        ]
        .into_iter()
        .collect();

        assert_eq!(dependencies, expected);
    }

    #[test]
    fn nested_expressions() {
        let template = r#"
        {% if user.is_active and user.age > 18 %}
            Welcome back, {{ user.name }}!
        {% else %}
            Please activate your account.
        {% endif %}
        "#;

        let mut dependencies = HashSet::new();
        parse_template_dependencies(template, &mut dependencies).unwrap();
        let expected: HashSet<StorePath> = vec![
            StorePath::from_dotted("user.is_active"),
            StorePath::from_dotted("user.age"),
            StorePath::from_dotted("user.name"),
        ]
        .into_iter()
        .collect();

        assert_eq!(dependencies, expected);
    }

    #[test]
    fn filter_expressions() {
        let template = r#"
        {{ products | filter(attribute="category", value="electronics") | map(attribute="price") | sum }}
        "#;

        let mut dependencies = HashSet::new();
        parse_template_dependencies(template, &mut dependencies).unwrap();
        let expected: HashSet<StorePath> = vec![StorePath::from_dotted("products")]
            .into_iter()
            .collect();

        assert_eq!(dependencies, expected);
    }

    #[test]
    fn for_loops() {
        let template = r#"
        {% for user in users %}
            {{ user.name }} - {{ user.email }}
        {% endfor %}
        "#;

        let mut dependencies = HashSet::new();
        parse_template_dependencies(template, &mut dependencies).unwrap();
        let expected: HashSet<StorePath> = vec![
            StorePath::from_dotted("users"),
            StorePath::from_dotted("user.name"),
            StorePath::from_dotted("user.email"),
        ]
        .into_iter()
        .collect();

        assert_eq!(dependencies, expected);
    }
}
