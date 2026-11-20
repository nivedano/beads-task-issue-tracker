//! Structural parsing of `.grace5/changes/<dir>/contract.xml`.
//!
//! Validation is structural only: XML parse failure, a root element that is not
//! `Contract`, a missing or empty `id`/`revision`/`title`, and an `id` that
//! disagrees with the change directory. Every section is read leniently, and
//! `grace5 check` remains authoritative for semantic defects.

use roxmltree::{Document, Node};

use super::model::{
    GraceAffected, GraceCheck, GraceConstraint, GraceContract, GraceCriterion, GraceEnvironment,
    GraceExpect, GraceItem,
};

/// The XSD `collapse` whitespace facet: trimmed, inner runs reduced to one space.
pub fn collapse(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

/// Every descendant text node in document order, concatenated, then collapsed.
pub fn element_text(el: &Node) -> String {
    let raw: String = el
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    collapse(&raw)
}

/// Parses the bytes of one change's `contract.xml`.
pub fn parse_contract(bytes: &[u8], dir_name: &str) -> Result<GraceContract, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("contract.xml is not valid UTF-8: {error}"))?;
    let document = Document::parse(text)
        .map_err(|error| format!("contract.xml is not well-formed XML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "Contract" {
        return Err(format!(
            "contract.xml root element is <{}>, expected <Contract>",
            root.tag_name().name()
        ));
    }

    let id = attribute(root, "id").unwrap_or_default();
    if id.is_empty() {
        return Err("contract.xml is missing the id attribute".to_string());
    }
    let revision_text = attribute(root, "revision").unwrap_or_default();
    if revision_text.is_empty() {
        return Err("contract.xml is missing the revision attribute".to_string());
    }
    let revision = revision_text.parse::<u32>().map_err(|_| {
        format!("contract.xml revision \"{revision_text}\" is not an unsigned number")
    })?;
    let title = attribute(root, "title").unwrap_or_default();
    if title.is_empty() {
        return Err("contract.xml is missing the title attribute".to_string());
    }
    if id != dir_name {
        return Err(format!(
            "contract id \"{id}\" does not match change directory \"{dir_name}\""
        ));
    }

    let checks_root = child_named(root, "Checks").unwrap_or(root);
    let environments = children_named(checks_root, "Environment")
        .into_iter()
        .map(|node| GraceEnvironment {
            id: attribute(node, "id").unwrap_or_default(),
            device: attribute(node, "device"),
            build: attribute(node, "build"),
            text: element_text(&node),
        })
        .collect();
    let checks = checks_root
        .children()
        .filter(|child| {
            child.is_element() && matches!(child.tag_name().name(), "Automated" | "Trace" | "Manual")
        })
        .map(build_check)
        .collect();

    Ok(GraceContract {
        id,
        revision,
        title,
        problem: text_of(root, "Problem"),
        change: text_of(root, "Change"),
        constraints: section(root, "Constraints", "Constraint")
            .into_iter()
            .map(|node| GraceConstraint {
                id: attribute(node, "id").unwrap_or_default(),
                text: element_text(&node),
                rule: attribute(node, "rule"),
            })
            .collect(),
        non_goals: section(root, "NonGoals", "NonGoal")
            .into_iter()
            .map(item)
            .collect(),
        acceptance: section(root, "Acceptance", "Criterion")
            .into_iter()
            .map(|node| GraceCriterion {
                id: attribute(node, "id").unwrap_or_default(),
                text: element_text(&node),
                checks: attribute(node, "checks")
                    .unwrap_or_default()
                    .split(' ')
                    .filter(|token| !token.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
            .collect(),
        affected: GraceAffected {
            paths: texts(root, "Affected", "Path"),
            names: texts(root, "Affected", "Name"),
            protected: texts(root, "Affected", "Protected"),
        },
        environments,
        checks,
        decisions: section(root, "Decisions", "Decision")
            .into_iter()
            .map(item)
            .collect(),
    })
}

fn build_check(node: Node) -> GraceCheck {
    let kind = node.tag_name().name().to_ascii_lowercase();
    let manual = kind == "manual";
    let trace = kind == "trace";
    GraceCheck {
        id: attribute(node, "id").unwrap_or_default(),
        environment: attribute(node, "environment"),
        cwd: if manual { None } else { attribute(node, "cwd") },
        budget: number(node.attribute("budget")),
        command: child_named(node, "Command").map(|child| element_text(&child)),
        procedure: child_named(node, "Procedure").map(|child| element_text(&child)),
        expects: children_named(node, "Expect")
            .into_iter()
            .map(|child| GraceExpect {
                text: element_text(&child),
                min: number(child.attribute("min")).unwrap_or(1),
                max: number(child.attribute("max")),
            })
            .collect(),
        forbids: children_named(node, "Forbid")
            .into_iter()
            .map(|child| element_text(&child))
            .collect(),
        origin: if trace { attribute(node, "origin") } else { None },
        log: if trace { attribute(node, "log") } else { None },
        required: xs_boolean(node.attribute("required")),
        kind,
    }
}

fn item(node: Node) -> GraceItem {
    GraceItem {
        id: attribute(node, "id").unwrap_or_default(),
        text: element_text(&node),
    }
}

fn child_named<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    parent
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn children_named<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Vec<Node<'a, 'input>> {
    parent
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == name)
        .collect()
}

fn section<'a, 'input>(
    root: Node<'a, 'input>,
    section: &str,
    child: &str,
) -> Vec<Node<'a, 'input>> {
    match child_named(root, section) {
        Some(parent) => children_named(parent, child),
        None => Vec::new(),
    }
}

fn texts(root: Node, section_name: &str, child: &str) -> Vec<String> {
    section(root, section_name, child)
        .into_iter()
        .map(|node| element_text(&node))
        .collect()
}

fn text_of(root: Node, name: &str) -> String {
    child_named(root, name)
        .map(|node| element_text(&node))
        .unwrap_or_default()
}

fn attribute(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(collapse)
}

fn number(value: Option<&str>) -> Option<u64> {
    value.and_then(|raw| collapse(raw).parse().ok())
}

/// The `xs:boolean` lexical space; anything but `true`/`1` is false.
fn xs_boolean(value: Option<&str>) -> bool {
    match value {
        Some(raw) => matches!(raw.trim(), "true" | "1"),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("fixtures/C-0019-retire-briefs/contract.xml");

    #[test]
    fn parses_the_c0019_fixture() {
        let contract =
            parse_contract(FIXTURE, "C-0019-retire-briefs").expect("fixture contract parses");
        assert_eq!(contract.id, "C-0019-retire-briefs");
        assert_eq!(contract.revision, 1);
        assert_eq!(
            contract.title,
            "The contract is the approval surface; the separate brief is retired"
        );
        assert_eq!(contract.constraints.len(), 8);
        assert_eq!(contract.acceptance.len(), 9);
        let kinds: Vec<(&str, &str)> = contract
            .checks
            .iter()
            .map(|check| (check.id.as_str(), check.kind.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("CK1", "automated"),
                ("CK2", "automated"),
                ("CK3", "automated"),
                ("CK4", "automated"),
                ("CK5", "trace"),
                ("CK6", "automated"),
                ("CK7", "automated"),
                ("CK8", "trace"),
            ]
        );
        assert!(contract
            .affected
            .protected
            .iter()
            .any(|path| path == ".grace5/kernel-version"));
        assert!(contract.problem.starts_with("The HTML brief was introduced"));
    }

    #[test]
    fn rejects_a_contract_without_title() {
        let src = br#"<Contract id="C-0001-x" revision="1"><Problem>p</Problem></Contract>"#;
        let err = parse_contract(src, "C-0001-x").expect_err("missing title must be rejected");
        assert!(
            err.contains("title"),
            "error should name the title attribute: {err}"
        );
    }

    #[test]
    fn rejects_an_id_that_does_not_match_the_directory() {
        let src =
            br#"<Contract id="C-0002-y" revision="1" title="T"><Problem>p</Problem></Contract>"#;
        let err = parse_contract(src, "C-0003-z").expect_err("id mismatch must be rejected");
        assert!(err.contains("id"), "error should name the id: {err}");
    }

    #[test]
    fn rejects_invalid_xml() {
        let src = br#"<Contract id="C-0004-w" revision="1" title="T"><Problem>"#;
        assert!(parse_contract(src, "C-0004-w").is_err());
    }

    #[test]
    fn preserves_non_ascii_text() {
        let src = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                   <Contract id=\"C-0042-unicode\" revision=\"2\" title=\"Café — 日本語\">\n\
                   <Problem>café — 日本語</Problem>\n\
                   </Contract>"
            .as_bytes();
        let contract = parse_contract(src, "C-0042-unicode").expect("non-ASCII contract parses");
        assert_eq!(contract.problem, "café — 日本語");
        assert_eq!(contract.title, "Café — 日本語");
        assert_eq!(contract.revision, 2);
    }

    #[test]
    fn collapse_trims_and_squeezes_runs() {
        assert_eq!(collapse("  a \t\n b  "), "a b");
        assert_eq!(collapse("one"), "one");
        assert_eq!(collapse(" \t\n "), "");
    }
}