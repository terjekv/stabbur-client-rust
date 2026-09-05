//! Reconciles reviewed resource specifications with the pinned OpenAPI contract.

use std::{collections::BTreeMap, env, fmt::Write as _, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OperationsDocument {
    operations: Vec<ContractOperation>,
}

#[derive(Debug, Deserialize)]
struct ContractOperation {
    method: String,
    operation_id: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct Specification {
    operations: Vec<ReviewedOperation>,
}

#[derive(Debug, Deserialize)]
struct ReviewedOperation {
    constant: String,
    method: String,
    operation_id: String,
    path: String,
    status: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = env::args().nth(1).unwrap_or_else(|| "check".to_owned());
    if !matches!(command.as_str(), "check" | "update") {
        return Err("usage: stabbur_reconcile [check|update]".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tool is a workspace child")
        .to_path_buf();
    let contract: OperationsDocument =
        serde_json::from_str(&fs::read_to_string(root.join("openapi/operations.json"))?)?;
    let specification: Specification = serde_json::from_str(&fs::read_to_string(
        root.join("stabbur_reconcile/specs/operations.json"),
    )?)?;

    let contract_by_id = contract
        .operations
        .into_iter()
        .map(|operation| (operation.operation_id.clone(), operation))
        .collect::<BTreeMap<_, _>>();
    let reviewed_by_id = specification
        .operations
        .iter()
        .map(|operation| (operation.operation_id.clone(), operation))
        .collect::<BTreeMap<_, _>>();
    if contract_by_id.keys().collect::<Vec<_>>() != reviewed_by_id.keys().collect::<Vec<_>>() {
        return Err(
            "reviewed resource specifications do not classify every OpenAPI operation".into(),
        );
    }
    for (operation_id, reviewed) in &reviewed_by_id {
        let contract = &contract_by_id[operation_id];
        if reviewed.method != contract.method || reviewed.path != contract.path {
            return Err(format!("reviewed specification drift for {operation_id}").into());
        }
        if !matches!(
            reviewed.status.as_str(),
            "implemented" | "planned" | "classified_protocol"
        ) {
            return Err(format!("invalid coverage status for {operation_id}").into());
        }
    }

    let generated = render(&specification.operations);
    let output = root.join("src/resources/generated.rs");
    if command == "update" {
        fs::write(&output, generated)?;
        println!("updated {}", output.display());
    } else if fs::read_to_string(&output)? != generated {
        return Err("generated resource code is stale; run update".into());
    } else {
        println!("reviewed resource specs cover every pinned OpenAPI operation");
    }
    Ok(())
}

fn render(operations: &[ReviewedOperation]) -> String {
    let mut output = String::from(concat!(
        "//! Generated operation constants. Do not edit by hand.\n\n",
        "/// One released HTTP operation.\n",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n",
        "pub struct Operation {\n",
        "    /// HTTP method.\n",
        "    pub method: &'static str,\n",
        "    /// Safe endpoint template.\n",
        "    pub path: &'static str,\n",
        "}\n\n",
    ));
    let mut sorted = operations.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|operation| &operation.constant);
    for operation in sorted {
        write!(
            output,
            concat!(
                "/// `{}`.\n",
                "pub const {}: Operation = Operation {{\n",
                "    method: {:?},\n",
                "    path: {:?},\n",
                "}};\n",
            ),
            operation.operation_id, operation.constant, operation.method, operation.path
        )
        .expect("writing generated Rust to a String is infallible");
    }
    output
}
