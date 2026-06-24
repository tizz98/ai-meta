//! GitHub Projects v2 via GraphQL. Best-effort: the classic REST API doesn't
//! cover Projects v2, and the token may lack the `project` scope, so callers
//! treat failures as non-fatal (warn + continue), exactly like the bash did.

use super::Github;
use crate::error::{Error, Result};
use serde_json::{json, Value};

/// Find a Projects v2 board by title under `owner`, creating it if missing.
/// Returns the project node id.
pub fn ensure_board(gh: &Github, owner: &str, title: &str) -> Result<String> {
    let (owner_id, existing) = owner_projects(gh, owner)?;
    if let Some(id) = existing
        .iter()
        .find(|(t, _)| t == title)
        .map(|(_, id)| id.clone())
    {
        return Ok(id);
    }
    create_project(gh, &owner_id, title)
}

/// Add a repo issue (by number) to a board. Best-effort.
pub fn add_issue(gh: &Github, project_id: &str, issue_number: u64) -> Result<()> {
    let node_id = issue_node_id(gh, issue_number)?;
    let q = r#"mutation($p:ID!,$c:ID!){ addProjectV2ItemById(input:{projectId:$p,contentId:$c}){ item { id } } }"#;
    let resp = gh.graphql(json!({"query": q, "variables": {"p": project_id, "c": node_id}}))?;
    check_errors(&resp)?;
    Ok(())
}

fn owner_projects(gh: &Github, owner: &str) -> Result<(String, Vec<(String, String)>)> {
    let q = r#"query($o:String!){ repositoryOwner(login:$o){ id ... on ProjectV2Owner { projectsV2(first:100){ nodes { id title } } } } }"#;
    let resp = gh.graphql(json!({"query": q, "variables": {"o": owner}}))?;
    check_errors(&resp)?;
    let owner_node = resp
        .pointer("/data/repositoryOwner")
        .ok_or_else(|| Error::msg(format!("owner {owner:?} not found")))?;
    let id = owner_node
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("owner has no node id"))?
        .to_string();
    let projects = owner_node
        .pointer("/projectsV2/nodes")
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    Some((
                        p.get("title")?.as_str()?.to_string(),
                        p.get("id")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((id, projects))
}

fn create_project(gh: &Github, owner_id: &str, title: &str) -> Result<String> {
    let q = r#"mutation($o:ID!,$t:String!){ createProjectV2(input:{ownerId:$o,title:$t}){ projectV2 { id } } }"#;
    let resp = gh.graphql(json!({"query": q, "variables": {"o": owner_id, "t": title}}))?;
    check_errors(&resp)?;
    resp.pointer("/data/createProjectV2/projectV2/id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| Error::msg("createProjectV2 returned no id"))
}

fn issue_node_id(gh: &Github, number: u64) -> Result<String> {
    let q = r#"query($o:String!,$r:String!,$n:Int!){ repository(owner:$o,name:$r){ issue(number:$n){ id } } }"#;
    let resp = gh.graphql(json!({"query": q, "variables": {
        "o": gh.repo.owner, "r": gh.repo.name, "n": number
    }}))?;
    check_errors(&resp)?;
    resp.pointer("/data/repository/issue/id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| Error::msg(format!("issue #{number} not found")))
}

fn check_errors(resp: &Value) -> Result<()> {
    if let Some(errors) = resp.get("errors").and_then(|e| e.as_array()) {
        if !errors.is_empty() {
            let msg = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(Error::msg(format!("GraphQL: {msg}")));
        }
    }
    Ok(())
}
