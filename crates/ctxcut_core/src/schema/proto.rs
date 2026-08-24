//! Protocol Buffers (`.proto`) schema parser and gRPC handler stitcher.

use crate::model::ExtractedType;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Extracted RPC definition inside a Protobuf service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoRpcDef {
    /// RPC method name (e.g. `ProcessOrder`).
    pub name: String,
    /// Request message type name (e.g. `OrderRequest`).
    pub request_type: String,
    /// Response message type name (e.g. `OrderResponse`).
    pub response_type: String,
    /// Client-side streaming flag.
    pub client_streaming: bool,
    /// Server-side streaming flag.
    pub server_streaming: bool,
    /// Verbatim RPC declaration snippet.
    pub signature: String,
}

/// Extracted Protobuf service definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoServiceDef {
    /// Service name (e.g. `OrderService`).
    pub name: String,
    /// Path to the defining `.proto` file.
    pub file_path: PathBuf,
    /// RPC methods declared in the service.
    pub rpcs: Vec<ProtoRpcDef>,
    /// Verbatim service block definition.
    pub definition: String,
}

/// Extracted Protobuf message definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoMessageDef {
    /// Message name (e.g. `OrderRequest`, `Outer`).
    pub name: String,
    /// Path to the defining `.proto` file.
    pub file_path: PathBuf,
    /// Field type names referenced inside the message.
    pub referenced_types: Vec<String>,
    /// Verbatim message block definition (including nested enums/messages).
    pub definition: String,
}

/// Extracted Protobuf top-level enum definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtoEnumDef {
    /// Enum name (e.g. `Status`).
    pub name: String,
    /// Path to the defining `.proto` file.
    pub file_path: PathBuf,
    /// Verbatim enum block definition.
    pub definition: String,
}

/// Parsed `.proto` file representation.
#[derive(Debug, Default, Clone)]
pub struct ParsedProtoFile {
    /// Services indexed by lowercase service name.
    pub services: HashMap<String, ProtoServiceDef>,
    /// Messages indexed by lowercase message name.
    pub messages: HashMap<String, ProtoMessageDef>,
    /// Enums indexed by lowercase enum name.
    pub enums: HashMap<String, ProtoEnumDef>,
}

/// Stitcher for Protobuf IDL definitions and gRPC service handlers.
#[derive(Debug, Default, Clone)]
pub struct ProtoStitcher;

impl ProtoStitcher {
    /// Creates a new `ProtoStitcher`.
    pub fn new() -> Self {
        Self
    }

    /// Discovers `.proto` files in the workspace, sorted by proximity to `current_file`.
    pub fn discover_proto_files(&self, workspace_root: &Path, current_file: &Path) -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        // 1. Check parent directory of current file
        if let Some(dir) = current_file.parent() {
            collect_proto_files(dir, &mut candidates, 2);
        }

        // 2. Check workspace standard directories
        let proto_dirs = [
            workspace_root.join("proto"),
            workspace_root.join("protos"),
            workspace_root.join("api").join("proto"),
            workspace_root.join("api"),
            workspace_root.join("src").join("proto"),
        ];
        for dir in &proto_dirs {
            if dir.is_dir() {
                collect_proto_files(dir, &mut candidates, 3);
            }
        }

        // 3. Fallback: Scan workspace
        if candidates.is_empty() {
            collect_proto_files(workspace_root, &mut candidates, 5);
        }

        let mut seen = HashSet::new();
        candidates.retain(|p| seen.insert(p.clone()));

        // Sort by proximity
        let target_dir = current_file.parent().unwrap_or(workspace_root);
        candidates.sort_by_key(|p| {
            let common_prefix_len = p
                .parent()
                .map(|parent| common_path_prefix_len(target_dir, parent))
                .unwrap_or(0);
            std::cmp::Reverse(common_prefix_len)
        });

        candidates
    }

    /// Parses a `.proto` file content into services, messages, and enums.
    pub fn parse_proto(&self, content: &str, file_path: &Path) -> ParsedProtoFile {
        let mut parsed = ParsedProtoFile::default();
        let lines: Vec<&str> = content.lines().collect();
        let mut idx = 0;

        while idx < lines.len() {
            let line = lines[idx].trim();
            if line.starts_with("//") || line.starts_with("/*") || line.is_empty() {
                idx += 1;
                continue;
            }

            if line.starts_with("service ") && line.contains('{') {
                let service_name = extract_block_name(line, "service");
                let (block_lines, next_idx) = extract_balanced_block(&lines, idx);
                idx = next_idx + 1;

                if let Some(name) = service_name {
                    let definition = block_lines.join("\n").trim().to_string();
                    let rpcs = parse_service_rpcs(&block_lines);
                    let service_def = ProtoServiceDef {
                        name: name.clone(),
                        file_path: file_path.to_path_buf(),
                        rpcs,
                        definition,
                    };
                    parsed.services.insert(name.to_lowercase(), service_def);
                }
                continue;
            } else if line.starts_with("message ") && line.contains('{') {
                let message_name = extract_block_name(line, "message");
                let (block_lines, next_idx) = extract_balanced_block(&lines, idx);
                idx = next_idx + 1;

                if let Some(name) = message_name {
                    let definition = block_lines.join("\n").trim().to_string();
                    let referenced_types = extract_message_field_types(&block_lines);
                    let message_def = ProtoMessageDef {
                        name: name.clone(),
                        file_path: file_path.to_path_buf(),
                        referenced_types,
                        definition,
                    };
                    parsed.messages.insert(name.to_lowercase(), message_def);
                }
                continue;
            } else if line.starts_with("enum ") && line.contains('{') {
                let enum_name = extract_block_name(line, "enum");
                let (block_lines, next_idx) = extract_balanced_block(&lines, idx);
                idx = next_idx + 1;

                if let Some(name) = enum_name {
                    let definition = block_lines.join("\n").trim().to_string();
                    let enum_def = ProtoEnumDef {
                        name: name.clone(),
                        file_path: file_path.to_path_buf(),
                        definition,
                    };
                    parsed.enums.insert(name.to_lowercase(), enum_def);
                }
                continue;
            }

            idx += 1;
        }

        parsed
    }

    /// Matches source symbols and identifiers against discovered Protobuf schemas and returns `ExtractedType` entries.
    pub fn stitch(
        &self,
        workspace_root: &Path,
        current_file: &Path,
        source: &str,
    ) -> Vec<ExtractedType> {
        let proto_files = self.discover_proto_files(workspace_root, current_file);
        if proto_files.is_empty() {
            return Vec::new();
        }

        let mut extracted = Vec::new();
        let mut seen_types = HashSet::new();

        for proto_path in &proto_files {
            let content = match fs::read_to_string(proto_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let parsed = self.parse_proto(&content, proto_path);

            // 1. Check services and RPC matching
            for service in parsed.services.values() {
                let mut matched_rpcs = Vec::new();
                for rpc in &service.rpcs {
                    let rpc_casing_variants = generate_name_variants(&rpc.name);
                    let is_matched = rpc_casing_variants.iter().any(|var| source.contains(var));

                    if is_matched {
                        matched_rpcs.push(rpc.clone());
                    }
                }

                if !matched_rpcs.is_empty() || source.contains(&service.name) {
                    if seen_types.insert(service.name.clone()) {
                        extracted.push(ExtractedType {
                            name: service.name.clone(),
                            kind: "protobuf_service".to_string(),
                            file_path: service.file_path.to_string_lossy().to_string(),
                            definition: service.definition.clone(),
                        });
                    }

                    // Hoist request and response messages for matched RPCs
                    for rpc in matched_rpcs {
                        hoist_proto_message_recursive(
                            &rpc.request_type,
                            &parsed,
                            &mut extracted,
                            &mut seen_types,
                            0,
                        );
                        hoist_proto_message_recursive(
                            &rpc.response_type,
                            &parsed,
                            &mut extracted,
                            &mut seen_types,
                            0,
                        );
                    }
                }
            }

            // 2. Check direct message matching in source
            for message in parsed.messages.values() {
                if source.contains(&message.name) {
                    hoist_proto_message_recursive(
                        &message.name,
                        &parsed,
                        &mut extracted,
                        &mut seen_types,
                        0,
                    );
                }
            }

            // 3. Check direct enum matching
            for proto_enum in parsed.enums.values() {
                if source.contains(&proto_enum.name) && seen_types.insert(proto_enum.name.clone()) {
                    extracted.push(ExtractedType {
                        name: proto_enum.name.clone(),
                        kind: "protobuf_enum".to_string(),
                        file_path: proto_enum.file_path.to_string_lossy().to_string(),
                        definition: proto_enum.definition.clone(),
                    });
                }
            }
        }

        extracted
    }
}

fn hoist_proto_message_recursive(
    type_name: &str,
    parsed: &ParsedProtoFile,
    extracted: &mut Vec<ExtractedType>,
    seen_types: &mut HashSet<String>,
    depth: usize,
) {
    if depth > 3 {
        return;
    }

    let clean_name = type_name.trim();
    if clean_name.is_empty() || is_builtin_proto_type(clean_name) {
        return;
    }

    let key = clean_name.to_lowercase();
    if let Some(msg) = parsed.messages.get(&key) {
        if seen_types.insert(msg.name.clone()) {
            extracted.push(ExtractedType {
                name: msg.name.clone(),
                kind: "protobuf_message".to_string(),
                file_path: msg.file_path.to_string_lossy().to_string(),
                definition: msg.definition.clone(),
            });

            for nested_ref in &msg.referenced_types {
                hoist_proto_message_recursive(nested_ref, parsed, extracted, seen_types, depth + 1);
            }
        }
    } else if let Some(e) = parsed.enums.get(&key) {
        if seen_types.insert(e.name.clone()) {
            extracted.push(ExtractedType {
                name: e.name.clone(),
                kind: "protobuf_enum".to_string(),
                file_path: e.file_path.to_string_lossy().to_string(),
                definition: e.definition.clone(),
            });
        }
    }
}

fn count_braces(line: &str) -> i64 {
    let mut diff: i64 = 0;
    for c in line.chars() {
        if c == '{' {
            diff += 1;
        } else if c == '}' {
            diff -= 1;
        }
    }
    diff
}

fn extract_block_name(line: &str, keyword: &str) -> Option<String> {
    let stripped = line.strip_prefix(keyword)?.trim_start();
    let name_part = stripped.split('{').next()?.trim();
    let name: String = name_part
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn extract_balanced_block<'a>(lines: &[&'a str], start_idx: usize) -> (Vec<&'a str>, usize) {
    let mut block = vec![lines[start_idx]];
    let mut brace_count: i64 = count_braces(lines[start_idx]);
    let mut idx = start_idx;

    while brace_count > 0 && idx + 1 < lines.len() {
        idx += 1;
        brace_count += count_braces(lines[idx]);
        block.push(lines[idx]);
    }

    (block, idx)
}

fn parse_service_rpcs(lines: &[&str]) -> Vec<ProtoRpcDef> {
    let mut rpcs = Vec::new();
    let joined = lines.join("\n");
    let inner = if let Some(open) = joined.find('{') {
        let after = &joined[open + 1..];
        if let Some(close) = after.rfind('}') {
            &after[..close]
        } else {
            after
        }
    } else {
        &joined
    };

    for statement in inner.split(';') {
        let trimmed = statement.trim();
        if let Some(rpc_start) = trimmed.find("rpc ") {
            let rpc_text = &trimmed[rpc_start..];
            if let Some(rpc) = parse_rpc_line(rpc_text) {
                rpcs.push(rpc);
            }
        }
    }
    rpcs
}

fn parse_rpc_line(line: &str) -> Option<ProtoRpcDef> {
    let after_rpc = line.strip_prefix("rpc ")?.trim_start();
    let open_paren = after_rpc.find('(')?;
    let rpc_name = after_rpc[..open_paren].trim().to_string();

    let after_req = &after_rpc[open_paren + 1..];
    let close_paren = after_req.find(')')?;
    let req_str = after_req[..close_paren].trim();
    let client_streaming = req_str.starts_with("stream ");
    let request_type = req_str
        .strip_prefix("stream ")
        .unwrap_or(req_str)
        .trim()
        .to_string();

    let returns_idx = after_req[close_paren..].find("returns")?;
    let after_returns = &after_req[close_paren + returns_idx + 7..];
    let resp_open = after_returns.find('(')?;
    let after_resp_open = &after_returns[resp_open + 1..];
    let resp_close = after_resp_open.find(')')?;
    let resp_str = after_resp_open[..resp_close].trim();
    let server_streaming = resp_str.starts_with("stream ");
    let response_type = resp_str
        .strip_prefix("stream ")
        .unwrap_or(resp_str)
        .trim()
        .to_string();

    Some(ProtoRpcDef {
        name: rpc_name,
        request_type,
        response_type,
        client_streaming,
        server_streaming,
        signature: line.trim().to_string(),
    })
}

fn extract_message_field_types(lines: &[&str]) -> Vec<String> {
    let mut types = Vec::new();
    let joined = lines.join("\n");
    let inner = if let Some(open) = joined.find('{') {
        let after = &joined[open + 1..];
        if let Some(close) = after.rfind('}') {
            &after[..close]
        } else {
            after
        }
    } else {
        &joined
    };

    for statement in inner.split(';') {
        let trimmed = statement.trim();
        if trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with("message ")
            || trimmed.starts_with("enum ")
            || trimmed.is_empty()
        {
            continue;
        }

        let clean_statement = if let Some(brace_idx) = trimmed.rfind('{') {
            trimmed[brace_idx + 1..].trim()
        } else {
            trimmed.trim_matches(['}', '{']).trim()
        };

        let parts: Vec<&str> = clean_statement.split_whitespace().collect();
        if parts.len() >= 2 {
            let type_token = if parts[0] == "repeated" || parts[0] == "optional" {
                if parts.len() >= 3 {
                    parts[1]
                } else {
                    parts[0]
                }
            } else {
                parts[0]
            };

            let clean_ty = type_token.trim_matches([';', ',', '{', '}']);
            if !is_builtin_proto_type(clean_ty) && !clean_ty.is_empty() && clean_ty != "oneof" {
                types.push(clean_ty.to_string());
            }
        }
    }
    types
}

fn is_builtin_proto_type(ty: &str) -> bool {
    matches!(
        ty,
        "double"
            | "float"
            | "int32"
            | "int64"
            | "uint32"
            | "uint64"
            | "sint32"
            | "sint64"
            | "fixed32"
            | "fixed64"
            | "sfixed32"
            | "sfixed64"
            | "bool"
            | "string"
            | "bytes"
            | "google.protobuf.Any"
            | "google.protobuf.Timestamp"
            | "google.protobuf.Empty"
    )
}

fn generate_name_variants(name: &str) -> Vec<String> {
    let mut variants = Vec::new();
    variants.push(name.to_string());

    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        let camel = format!("{}{}", first.to_lowercase(), chars.as_str());
        if !variants.contains(&camel) {
            variants.push(camel);
        }
    }

    let mut snake = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }
    if !variants.contains(&snake) {
        variants.push(snake);
    }

    variants
}

fn collect_proto_files(dir: &Path, results: &mut Vec<PathBuf>, max_depth: usize) {
    if max_depth == 0 {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !matches!(
                    name,
                    "node_modules"
                        | "target"
                        | ".git"
                        | ".agents"
                        | ".ctxcut"
                        | "dist"
                        | "build"
                        | "vendor"
                        | ".cache"
                        | ".next"
                ) {
                    collect_proto_files(&p, results, max_depth - 1);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension() {
                    if ext == "proto" {
                        results.push(p);
                    }
                }
            }
        }
    }
}

fn common_path_prefix_len(p1: &Path, p2: &Path) -> usize {
    let c1: Vec<_> = p1.components().collect();
    let c2: Vec<_> = p2.components().collect();
    c1.iter().zip(c2.iter()).take_while(|(a, b)| a == b).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_proto_service_and_messages() {
        let content = r#"
syntax = "proto3";
package orders;

message OrderRequest {
    string order_id = 1;
    double amount = 2;
}

message OrderResponse {
    bool success = 1;
}

service OrderService {
    rpc ProcessOrder(OrderRequest) returns (OrderResponse);
}
"#;
        let stitcher = ProtoStitcher::new();
        let parsed = stitcher.parse_proto(content, Path::new("service.proto"));

        assert!(parsed.services.contains_key("orderservice"));
        assert!(parsed.messages.contains_key("orderrequest"));
        assert!(parsed.messages.contains_key("orderresponse"));

        let service = parsed.services.get("orderservice").unwrap();
        assert_eq!(service.name, "OrderService");
        assert_eq!(service.rpcs.len(), 1);
        assert_eq!(service.rpcs[0].name, "ProcessOrder");
        assert_eq!(service.rpcs[0].request_type, "OrderRequest");
        assert_eq!(service.rpcs[0].response_type, "OrderResponse");
    }

    #[test]
    fn test_stitch_proto_grpc_handler() {
        let temp_dir = TempDir::new().unwrap();
        let proto_path = temp_dir.path().join("service.proto");
        let content = r#"
syntax = "proto3";

message OrderRequest {
    string id = 1;
}

message OrderResponse {
    bool ok = 1;
}

service OrderService {
    rpc ProcessOrder(OrderRequest) returns (OrderResponse);
}
"#;
        fs::write(&proto_path, content).unwrap();

        let source = "async fn process_order(&self, req: OrderRequest) -> Result<OrderResponse> { Ok(OrderResponse { ok: true }) }";
        let stitcher = ProtoStitcher::new();
        let file_path = temp_dir.path().join("src/handler.rs");

        let stitched = stitcher.stitch(temp_dir.path(), &file_path, source);
        assert!(stitched
            .iter()
            .any(|t| t.name == "OrderService" && t.kind == "protobuf_service"));
        assert!(stitched
            .iter()
            .any(|t| t.name == "OrderRequest" && t.kind == "protobuf_message"));
        assert!(stitched
            .iter()
            .any(|t| t.name == "OrderResponse" && t.kind == "protobuf_message"));
    }
}
