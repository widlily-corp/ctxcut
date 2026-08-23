//! Empirical Challenger 2 Verification Suite for Milestone 1 Feature 3: Polyglot Interface & Trait Implementor Hoisting.
//!
//! Tests:
//! 1. Rust: `impl Trait for Struct`, generic traits (`Trait<T>`), qualified paths (`crate::path::Trait`), where clauses.
//! 2. Go: Structural duck typing:
//!    - Empty `interface{}` and `any` must NOT match all structs.
//!    - Pointer receiver `(s *Struct)` vs value receiver `(s Struct)`.
//!    - Partial method sets must NOT match.
//!    - Superset method sets (all required + extra) MUST match.
//! 3. TypeScript: `class C implements I1, I2<T>` with multi-interfaces and generic parameters.
//! 4. Python:
//!    - Nominal subclassing (`class Impl(Protocol)`).
//!    - Structural duck typing (`class Impl` with matching method names).
//! 5. Performance / Latency verification:
//!    - `ImplementorHoister::hoist_implementors_for_slice` execution latency benchmark.

#![allow(clippy::needless_raw_string_hashes)]

use ctxcut_core::model::{SliceOptions, SupportedLanguage};
use ctxcut_core::resolver::ImplementorHoister;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn test_empirical_rust_implementor_hoisting_comprehensive() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let trait_file = ws.join("service.rs");
    fs::write(
        &trait_file,
        r#"
pub trait StorageEngine<T: Clone> {
    fn put(&mut self, key: &str, val: T) -> Result<(), String>;
    fn get(&self, key: &str) -> Option<T>;
}

pub fn execute_store<T: Clone>(engine: &mut dyn StorageEngine<T>, key: &str, val: T) -> Result<(), String> {
    engine.put(key, val)
}
"#,
    )
    .expect("write trait");

    let impl_file = ws.join("redis.rs");
    fs::write(
        &impl_file,
        r#"
use crate::service::StorageEngine;

pub struct RedisStore {
    pub host: String,
}

impl<T: Clone + Send> StorageEngine<T> for RedisStore {
    fn put(&mut self, key: &str, val: T) -> Result<(), String> {
        println!("Stored key {}", key);
        Ok(())
    }

    fn get(&self, key: &str) -> Option<T> {
        None
    }
}
"#,
    )
    .expect("write impl");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let start = Instant::now();
    let slice = slicer
        .slice_symbol(&trait_file, "execute_store", &opts)
        .expect("slice symbol");
    let elapsed = start.elapsed();

    println!("Rust implementor hoisting elapsed: {:?}", elapsed);
    assert!(!slice.hoisted_implementors.is_empty(), "Must hoist RedisStore implementor");
    let imp = &slice.hoisted_implementors[0];
    assert_eq!(imp.interface_name, "StorageEngine");
    assert_eq!(imp.implementor_name, "RedisStore");
    assert_eq!(imp.kind, "rust_impl");
    assert!(imp.definition.contains("impl<T: Clone + Send> StorageEngine<T> for RedisStore"));
    assert!(imp.definition.contains("fn put"));
    assert!(imp.definition.contains("fn get"));
}

#[test]
fn test_empirical_go_duck_typing_empty_interface_and_receivers() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let iface_file = ws.join("handler.go");
    fs::write(
        &iface_file,
        r#"
package main

type EmptyIface interface{}
type AnyAlias any

type Writer interface {
    Write(p []byte) (n int, err error)
    Sync() error
}

func HandleEmpty(val EmptyIface) {}
func HandleAny(val AnyAlias) {}
func ExecuteWrite(w Writer, data []byte) (int, error) {
    return w.Write(data)
}
"#,
    )
    .expect("write iface");

    let impl_file = ws.join("impls.go");
    fs::write(
        &impl_file,
        r#"
package main

// Complete implementor with pointer receiver
type FileWriter struct {
    Path string
}

func (f *FileWriter) Write(p []byte) (n int, err error) {
    return len(p), nil
}

func (f *FileWriter) Sync() error {
    return nil
}

// Complete implementor with value receiver + extra method
type MemoryWriter struct {
    Buffer []byte
}

func (m MemoryWriter) Write(p []byte) (n int, err error) {
    return len(p), nil
}

func (m MemoryWriter) Sync() error {
    return nil
}

func (m MemoryWriter) Reset() {
    // extra method
}

// Partial implementor (only Write, missing Sync) - MUST NOT MATCH
type IncompleteWriter struct{}

func (i *IncompleteWriter) Write(p []byte) (n int, err error) {
    return 0, nil
}

// Random struct without methods - MUST NOT MATCH
type RandomData struct {
    Count int
}
"#,
    )
    .expect("write impls");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // 1. Empty interface must NOT match all structs
    let slice_empty = slicer
        .slice_symbol(&iface_file, "HandleEmpty", &opts)
        .expect("slice HandleEmpty");
    assert!(
        slice_empty.hoisted_implementors.is_empty(),
        "Empty interface must NOT match all structs"
    );

    // 2. AnyAlias must NOT match all structs
    let slice_any = slicer
        .slice_symbol(&iface_file, "HandleAny", &opts)
        .expect("slice HandleAny");
    assert!(
        slice_any.hoisted_implementors.is_empty(),
        "Any alias must NOT match all structs"
    );

    // 3. Writer interface matching
    let slice_writer = slicer
        .slice_symbol(&iface_file, "ExecuteWrite", &opts)
        .expect("slice ExecuteWrite");

    let implementor_names: Vec<String> = slice_writer
        .hoisted_implementors
        .iter()
        .map(|imp| imp.implementor_name.clone())
        .collect();

    println!("Discovered Go implementors for Writer: {:?}", implementor_names);
    assert!(implementor_names.contains(&"FileWriter".to_string()), "FileWriter (pointer receiver) must match");
    assert!(implementor_names.contains(&"MemoryWriter".to_string()), "MemoryWriter (value receiver + extra) must match");
    assert!(!implementor_names.contains(&"IncompleteWriter".to_string()), "IncompleteWriter (missing Sync) must NOT match");
    assert!(!implementor_names.contains(&"RandomData".to_string()), "RandomData must NOT match");
}

#[test]
fn test_empirical_typescript_multi_implements_and_generics() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let iface_file = ws.join("contracts.ts");
    fs::write(
        &iface_file,
        r#"
export interface Serializable<T> {
    serialize(): string;
    deserialize(data: string): T;
}

export interface Auditable {
    getAuditId(): string;
}

export function processItem<T>(item: Serializable<T>): string {
    return item.serialize();
}
"#,
    )
    .expect("write contracts");

    let impl_file = ws.join("models.ts");
    fs::write(
        &impl_file,
        r#"
import { Serializable, Auditable } from './contracts';

export class UserRecord<T = string> implements Serializable<T>, Auditable {
    public serialize(): string {
        return "{}";
    }

    public deserialize(data: string): T {
        return {} as T;
    }

    public getAuditId(): string {
        return "audit_123";
    }
}

export class UnrelatedClass {
    public run(): void {}
}
"#,
    )
    .expect("write models");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&iface_file, "processItem", &opts)
        .expect("slice processItem");

    let implementor_names: Vec<String> = slice
        .hoisted_implementors
        .iter()
        .map(|imp| imp.implementor_name.clone())
        .collect();

    assert!(implementor_names.contains(&"UserRecord".to_string()), "UserRecord implementing multiple interfaces must match");
    assert!(!implementor_names.contains(&"UnrelatedClass".to_string()), "UnrelatedClass must NOT match");
}

#[test]
fn test_empirical_python_nominal_and_protocol_duck_typing() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let proto_file = ws.join("protocol_suite.py");
    fs::write(
        &proto_file,
        r#"
from typing import Protocol

class EventSink(Protocol):
    def emit_event(self, event_type: str, payload: dict) -> bool:
        ...

# 1. Nominal implementor in same file
class KafkaEventSink(EventSink):
    def emit_event(self, event_type: str, payload: dict) -> bool:
        return True

# 2. Structural duck-typed implementor in same file
class RabbitMQEventSink:
    def emit_event(self, event_type: str, payload: dict) -> bool:
        return True

# 3. Non-implementor (missing method)
class NullSink:
    def close(self) -> None:
        pass

def dispatch_event(sink: EventSink, name: str, data: dict) -> bool:
    return sink.emit_event(name, data)
"#,
    )
    .expect("write python proto");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&proto_file, "dispatch_event", &opts)
        .expect("slice dispatch_event");

    let implementor_names: Vec<String> = slice
        .hoisted_implementors
        .iter()
        .map(|imp| imp.implementor_name.clone())
        .collect();

    println!("Discovered Python implementors: {:?}", implementor_names);
    assert!(implementor_names.contains(&"KafkaEventSink".to_string()), "KafkaEventSink (nominal) must match");
    assert!(implementor_names.contains(&"RabbitMQEventSink".to_string()), "RabbitMQEventSink (structural duck-typed) must match");
    assert!(!implementor_names.contains(&"NullSink".to_string()), "NullSink must NOT match");
}

#[test]
fn test_empirical_implementor_hoisting_latency_benchmarks() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let target_file = ws.join("service_0.rs");
    fs::write(
        &target_file,
        r#"
pub trait Service_0 {
    fn execute_0(&self) -> u32;
}

pub struct Worker_0;

impl Service_0 for Worker_0 {
    fn execute_0(&self) -> u32 { 0 }
}

pub fn run_service_0(s: &dyn Service_0) -> u32 {
    s.execute_0()
}
"#,
    )
    .expect("write service file");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    // Warmup
    let _ = slicer.slice_symbol(&target_file, "run_service_0", &opts);

    // Measure pure ImplementorHoister latency
    let slice = slicer
        .slice_symbol(&target_file, "run_service_0", &opts)
        .expect("slice run_service_0");

    assert!(!slice.hoisted_implementors.is_empty());
    assert_eq!(slice.hoisted_implementors[0].implementor_name, "Worker_0");

    // Hoister direct call timing
    let hoister_start = Instant::now();
    let imps = ImplementorHoister::hoist_implementors_for_slice(
        ws,
        &target_file,
        &slice.target_symbol,
        &slice.hoisted_types,
        SupportedLanguage::Rust,
    )
    .expect("hoist implementors");
    let hoister_elapsed = hoister_start.elapsed();

    println!("Direct ImplementorHoister::hoist_implementors_for_slice elapsed: {:?}", hoister_elapsed);
    assert!(!imps.is_empty());
    assert!(
        hoister_elapsed.as_millis() < 50,
        "ImplementorHoister latency must be fast, took {:?}",
        hoister_elapsed
    );
}
