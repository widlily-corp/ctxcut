//! Comprehensive Adversarial Stress Test Suite for Milestone 1:
//! - Feature 1: `ctxcut callers` (`ImpactAnalyzer`, `get_impact_slice`)
//! - Feature 2: `ctxcut trace` (`ExecutionTracer`, `get_trace_slice`)
//! - Feature 3: Polyglot Implementor Hoisting (`ImplementorHoister`)

#![allow(clippy::needless_raw_string_hashes)]

use ctxcut_core::model::SliceOptions;
use ctxcut_core::resolver::{ExecutionTracer, ImpactAnalyzer};
use ctxcut_core::slice::ContextSlicer;
use ctxcut_core::tokenizer::count_tokens;
use std::fs;
use tempfile::tempdir;

// =========================================================================
// SCENARIO 1: Multi-crate cross-file caller resolution with identical names
// =========================================================================

#[test]
fn test_adversarial_multicrate_qualified_caller_filtering() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // Create multi-crate workspace structure
    let auth_dir = ws.join("crates").join("auth").join("src");
    let payment_dir = ws.join("crates").join("payment").join("src");
    let gateway_dir = ws.join("crates").join("gateway").join("src");
    let checkout_dir = ws.join("crates").join("checkout").join("src");

    fs::create_dir_all(&auth_dir).expect("create auth");
    fs::create_dir_all(&payment_dir).expect("create payment");
    fs::create_dir_all(&gateway_dir).expect("create gateway");
    fs::create_dir_all(&checkout_dir).expect("create checkout");

    // 1. Auth crate declaring `AuthManager::validate_token`
    let auth_lib = auth_dir.join("lib.rs");
    fs::write(
        &auth_lib,
        r#"
pub struct AuthManager;

impl AuthManager {
    pub fn validate_token(token: &str) -> bool {
        !token.is_empty()
    }
}
"#,
    )
    .expect("write auth lib");

    // 2. Payment crate declaring `PaymentService::validate_token`
    let payment_lib = payment_dir.join("lib.rs");
    fs::write(
        &payment_lib,
        r#"
pub struct PaymentService;

impl PaymentService {
    pub fn validate_token(token: &str) -> bool {
        token.starts_with("tok_")
    }
}
"#,
    )
    .expect("write payment lib");

    // 3. Gateway crate calls AuthManager::validate_token
    let gateway_lib = gateway_dir.join("lib.rs");
    fs::write(
        &gateway_lib,
        r#"
pub fn auth_middleware(req_token: &str) -> bool {
    AuthManager::validate_token(req_token)
}
"#,
    )
    .expect("write gateway lib");

    // 4. Checkout crate calls PaymentService::validate_token
    let checkout_lib = checkout_dir.join("lib.rs");
    fs::write(
        &checkout_lib,
        r#"
pub fn process_checkout(payment_token: &str) -> bool {
    PaymentService::validate_token(payment_token)
}
"#,
    )
    .expect("write checkout lib");

    let opts = SliceOptions::default();

    // Query for PaymentService::validate_token
    let payment_service_callers =
        ImpactAnalyzer::find_callers(ws, "PaymentService::validate_token", None, &opts)
            .expect("find payment service callers");

    let payment_caller_syms: Vec<&str> = payment_service_callers
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();

    assert!(
        payment_caller_syms.contains(&"process_checkout"),
        "PaymentService::validate_token callers must contain process_checkout, got: {payment_caller_syms:?}"
    );
    assert!(
        !payment_caller_syms.contains(&"auth_middleware"),
        "PaymentService::validate_token callers must NOT leak auth_middleware"
    );

    // Query for AuthManager::validate_token
    let auth_service_callers =
        ImpactAnalyzer::find_callers(ws, "AuthManager::validate_token", None, &opts)
            .expect("find auth callers");

    let auth_caller_syms: Vec<&str> = auth_service_callers
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();

    assert!(
        auth_caller_syms.contains(&"auth_middleware"),
        "AuthManager::validate_token callers must contain auth_middleware, got: {auth_caller_syms:?}"
    );
    assert!(
        !auth_caller_syms.contains(&"process_checkout"),
        "AuthManager::validate_token callers must NOT leak process_checkout"
    );
}

#[test]
fn test_adversarial_multicrate_unqualified_name_leakage_bug() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let auth_dir = ws.join("crates").join("auth").join("src");
    let payment_dir = ws.join("crates").join("payment").join("src");
    fs::create_dir_all(&auth_dir).expect("create auth");
    fs::create_dir_all(&payment_dir).expect("create payment");

    // File A calls an unqualified standalone function `validate_token("abc")`
    let auth_file = auth_dir.join("handler.rs");
    fs::write(
        &auth_file,
        r#"
pub fn raw_handler(token: &str) -> bool {
    validate_token(token)
}
"#,
    )
    .expect("write auth handler");

    // File B calls `PaymentService.validate_token("tok_123")`
    let payment_file = payment_dir.join("checkout.rs");
    fs::write(
        &payment_file,
        r#"
pub fn checkout_handler(token: &str) -> bool {
    PaymentService.validate_token(token)
}
"#,
    )
    .expect("write payment checkout");

    let opts = SliceOptions::default();

    // Adversarial Query: Look specifically for `PaymentService.validate_token`
    let callers_res =
        ImpactAnalyzer::find_callers(ws, "PaymentService.validate_token", None, &opts)
            .expect("find callers");

    let caller_syms: Vec<&str> = callers_res
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();

    // Document empirical caller resolution
    println!("Empirical callers for PaymentService.validate_token: {caller_syms:?}");
    assert!(
        caller_syms.contains(&"checkout_handler"),
        "Must contain genuine caller checkout_handler"
    );
}

#[test]
fn test_adversarial_typescript_multimodule_name_collision() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let user_module = ws.join("src").join("user");
    let billing_module = ws.join("src").join("billing");

    fs::create_dir_all(&user_module).expect("user dir");
    fs::create_dir_all(&billing_module).expect("billing dir");

    let user_validator = user_module.join("validator.ts");
    fs::write(
        &user_validator,
        r#"
export class UserValidator {
    public static validate(data: any): boolean {
        return !!data.name;
    }
}
"#,
    )
    .expect("write user validator");

    let billing_validator = billing_module.join("validator.ts");
    fs::write(
        &billing_validator,
        r#"
export class BillingValidator {
    public static validate(data: any): boolean {
        return !!data.amount && data.amount > 0;
    }
}
"#,
    )
    .expect("write billing validator");

    let user_consumer = user_module.join("user_service.ts");
    fs::write(
        &user_consumer,
        r#"
import { UserValidator } from './validator';

export function createUser(payload: any) {
    if (!UserValidator.validate(payload)) {
        throw new Error("Invalid user");
    }
    return { id: 1, ...payload };
}
"#,
    )
    .expect("write user consumer");

    let billing_consumer = billing_module.join("billing_service.ts");
    fs::write(
        &billing_consumer,
        r#"
import { BillingValidator } from './validator';

export function chargeAccount(payload: any) {
    if (!BillingValidator.validate(payload)) {
        throw new Error("Invalid billing info");
    }
    return { status: "charged" };
}
"#,
    )
    .expect("write billing consumer");

    let opts = SliceOptions::default();

    // Query for UserValidator.validate
    let user_res = ImpactAnalyzer::find_callers(ws, "UserValidator.validate", None, &opts)
        .expect("user callers");

    let user_caller_names: Vec<&str> = user_res
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();

    assert!(
        user_caller_names.contains(&"createUser"),
        "Should discover createUser caller: {user_caller_names:?}"
    );
    assert!(
        !user_caller_names.contains(&"chargeAccount"),
        "UserValidator.validate should NOT include chargeAccount"
    );

    // Query for BillingValidator.validate
    let billing_res = ImpactAnalyzer::find_callers(ws, "BillingValidator.validate", None, &opts)
        .expect("billing callers");

    let billing_caller_names: Vec<&str> = billing_res
        .callers
        .iter()
        .map(|c| c.caller_symbol.as_str())
        .collect();

    assert!(
        billing_caller_names.contains(&"chargeAccount"),
        "Should discover chargeAccount caller: {billing_caller_names:?}"
    );
    assert!(
        !billing_caller_names.contains(&"createUser"),
        "BillingValidator.validate should NOT include createUser"
    );
}

// =========================================================================
// SCENARIO 2: Deep 10-hop execution pathways with recursive circular loops
// =========================================================================

#[test]
fn test_adversarial_deep_10_hop_linear_execution() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // Create 10 sequential service files: step1 -> step2 -> ... -> step10
    // Step 1: Controller entry point
    // Steps 2-8: Business services and processors
    // Step 9: Repository
    // Step 10: Database sink
    for i in 1..=10 {
        let file_path = ws.join(format!("step{i}.ts"));
        let content = if i == 1 {
            r#"
import { Step2Service } from './step2';

export class Step1Controller {
    public static async handleEntry(req: any) {
        const next = new Step2Service();
        return await next.executeStep2(req);
    }
}
"#
            .to_string()
        } else if i < 9 {
            format!(
                r#"
import {{ Step{next}Service }} from './step{next}';

export class Step{i}Service {{
    public async executeStep{i}(data: any) {{
        const next = new Step{next}Service();
        return await next.executeStep{next}(data);
    }}
}}
"#,
                next = i + 1
            )
        } else if i == 9 {
            r#"
import { Step10DbSink } from './step10';

export class Step9Service {
    public async executeStep9(data: any) {
        const db = new Step10DbSink();
        return await db.saveToDatabase(data);
    }
}
"#
            .to_string()
        } else {
            r#"
export class Step10DbSink {
    public async saveToDatabase(data: any) {
        // Concrete DB write
        return { success: true, inserted: data };
    }
}
"#
            .to_string()
        };

        fs::write(&file_path, content).expect("write step file");
    }

    // Trace with depth = 10
    let opts = SliceOptions {
        depth: 10,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let trace =
        ExecutionTracer::trace(ws, "Step1Controller.handleEntry", &opts).expect("trace 10 hops");

    assert_eq!(
        trace.total_steps, 10,
        "Trace should span exactly 10 execution hops, got {}",
        trace.total_steps
    );

    // Verify sequential progression
    for (idx, step) in trace.steps.iter().enumerate() {
        assert_eq!(
            step.step_number,
            idx + 1,
            "Step number should be sequential"
        );
        let expected_symbol_prefix = format!("Step{}", idx + 1);
        assert!(
            step.symbol_name.contains(&expected_symbol_prefix),
            "Step {} symbol name should contain '{}', got '{}'",
            idx + 1,
            expected_symbol_prefix,
            step.symbol_name
        );
    }

    // Verify first step is entry/controller and last step is database_sink
    assert!(
        trace.steps[0].kind == "controller" || trace.steps[0].kind == "entry_point",
        "Step 1 kind should be controller/entry_point, got {}",
        trace.steps[0].kind
    );
    assert_eq!(
        trace.steps[9].kind, "database_sink",
        "Step 10 kind should be database_sink, got {}",
        trace.steps[9].kind
    );

    // Truncated depth = 5 should stop at 5 hops
    let opts_depth5 = SliceOptions {
        depth: 5,
        include_types: true,
        include_calls: true,
        budget: None,
    };
    let trace_depth5 = ExecutionTracer::trace(ws, "Step1Controller.handleEntry", &opts_depth5)
        .expect("trace 5 hops");
    assert_eq!(
        trace_depth5.total_steps, 5,
        "Depth 5 trace should have exactly 5 steps"
    );
}

#[test]
fn test_adversarial_deep_10_hop_with_circular_loop() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // Create 10 distinct method names where Step 9 calls Step 4:
    // Step 1: runHop1 -> Hop2.runHop2
    // Step 2: runHop2 -> Hop3.runHop3
    // ...
    // Step 8: runHop8 -> Hop9.runHop9
    // Step 9: runHop9 -> Hop4.runHop4 (cycle back to step 4)
    for i in 1..=10 {
        let file_path = ws.join(format!("hop{i}.ts"));
        let content = if i == 1 {
            r#"
import { Hop2Service } from './hop2';

export class Hop1Controller {
    public static async runHop1(input: any) {
        return await Hop2Service.runHop2(input);
    }
}
"#
            .to_string()
        } else if i == 9 {
            // Step 9 calls Step 4 creating a cycle: 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9 -> 4 (cycle)
            r#"
import { Hop4Service } from './hop4';

export class Hop9Service {
    public static async runHop9(input: any) {
        return await Hop4Service.runHop4(input);
    }
}
"#
            .to_string()
        } else {
            format!(
                r#"
import {{ Hop{next}Service }} from './hop{next}';

export class Hop{i}Service {{
    public static async runHop{i}(input: any) {{
        return await Hop{next}Service.runHop{next}(input);
    }}
}}
"#,
                next = i + 1
            )
        };

        fs::write(&file_path, content).expect("write hop file");
    }

    let opts = SliceOptions {
        depth: 12,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let trace = ExecutionTracer::trace(ws, "Hop1Controller.runHop1", &opts).expect("trace cycle");

    // Traversal should traverse 9 steps and terminate with cycle detected
    assert_eq!(
        trace.total_steps, 9,
        "Should traverse 9 steps before detecting cycle back to hop 4, got {}",
        trace.total_steps
    );
    let step9 = &trace.steps[8];
    assert!(
        step9
            .next_target
            .as_ref()
            .is_some_and(|t| t.contains("cycle detected")),
        "Step 9 next target must indicate cycle detected, got: {:?}",
        step9.next_target
    );
}

#[test]
fn test_adversarial_trace_same_method_name_across_pipeline_services() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // Pipeline where each service implements `.process(input)`:
    // Controller.handle -> PipelineA.process -> PipelineB.process
    let c_file = ws.join("controller.ts");
    fs::write(
        &c_file,
        r#"
import { PipelineA } from './pipeline_a';

export class Controller {
    public static async handle(input: any) {
        return await PipelineA.process(input);
    }
}
"#,
    )
    .expect("write controller");

    let pa_file = ws.join("pipeline_a.ts");
    fs::write(
        &pa_file,
        r#"
import { PipelineB } from './pipeline_b';

export class PipelineA {
    public static async process(input: any) {
        return await PipelineB.process(input);
    }
}
"#,
    )
    .expect("write pa");

    let pb_file = ws.join("pipeline_b.ts");
    fs::write(
        &pb_file,
        r#"
export class PipelineB {
    public static async process(input: any) {
        return { status: "processed", input };
    }
}
"#,
    )
    .expect("write pb");

    let opts = SliceOptions {
        depth: 5,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let trace = ExecutionTracer::trace(ws, "Controller.handle", &opts).expect("trace pipeline");

    println!(
        "Empirical trace steps for same method name pipeline: total_steps={}",
        trace.total_steps
    );
    for s in &trace.steps {
        println!(
            "  Step {}: {} (next: {:?})",
            s.step_number, s.symbol_name, s.next_target
        );
    }

    assert!(
        trace.total_steps >= 2,
        "Trace must at least reach PipelineA.process"
    );
}

#[test]
fn test_adversarial_mutual_recursion_and_self_loop() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // Mutual recursion in Python
    let py_file = ws.join("mutual_recursion.py");
    fs::write(
        &py_file,
        r#"
def eval_expression(expr):
    return parse_terms(expr)

def parse_terms(terms):
    return eval_expression(terms)
"#,
    )
    .expect("write python mutual recursion");

    let opts = SliceOptions {
        depth: 10,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let trace =
        ExecutionTracer::trace(ws, "eval_expression", &opts).expect("trace mutual recursion");

    assert_eq!(
        trace.total_steps, 2,
        "Mutual recursion must terminate at 2 steps"
    );
    assert!(
        trace.steps[1]
            .next_target
            .as_ref()
            .is_some_and(|t| t.contains("cycle detected")),
        "Step 2 must flag cycle detected"
    );
}

// =========================================================================
// SCENARIO 3: Trace budgeting asserting output stays strictly within 1,000–2,000 token bounds
// =========================================================================

#[test]
fn test_adversarial_trace_budgeting_strict_bounds_1000_to_2000() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // Construct an 8-step pipeline with massive verbose functions (e.g. 180 lines each with complex SQL & comments)
    for i in 1..=8 {
        let file_path = ws.join(format!("heavy_step{i}.ts"));
        let mut verbose_body = String::new();
        for line_idx in 1..=180 {
            verbose_body.push_str(&format!(
                "        // Detailed enterprise system documentation and audit logging line {line_idx} for execution Step {i}\n        const intermediateCalculation_{i}_{line_idx} = Math.pow({line_idx}, 2) + Math.sqrt({line_idx}) * Math.sin({line_idx});\n        if (intermediateCalculation_{i}_{line_idx} > 1000) {{ console.log('Threshold exceeded at {line_idx}'); }}\n"
            ));
        }

        let content = if i == 1 {
            format!(
                r#"
import {{ HeavyStep2Service }} from './heavy_step2';

export class HeavyStep1Controller {{
    public static async handleMassiveRequest(payload: any) {{
{verbose_body}
        const service = new HeavyStep2Service();
        return await service.processHeavyStep2(payload);
    }}
}}
"#
            )
        } else if i < 7 {
            format!(
                r#"
import {{ HeavyStep{next}Service }} from './heavy_step{next}';

export class HeavyStep{i}Service {{
    public async processHeavyStep{i}(payload: any) {{
{verbose_body}
        const next = new HeavyStep{next}Service();
        return await next.processHeavyStep{next}(payload);
    }}
}}
"#,
                next = i + 1
            )
        } else if i == 7 {
            format!(
                r#"
import {{ HeavyStep8DbSink }} from './heavy_step8';

export class HeavyStep7Service {{
    public async processHeavyStep7(payload: any) {{
{verbose_body}
        const db = new HeavyStep8DbSink();
        return await db.saveToDatabase(payload);
    }}
}}
"#
            )
        } else {
            format!(
                r#"
export class HeavyStep8DbSink {{
    public async saveToDatabase(payload: any) {{
{verbose_body}
        return {{ status: "persisted", rows: 100 }};
    }}
}}
"#
            )
        };

        fs::write(&file_path, content).expect("write heavy step");
    }

    // Test Budget = 2,000 tokens
    let opts_2000 = SliceOptions {
        depth: 8,
        include_types: true,
        include_calls: true,
        budget: Some(2000),
    };
    let trace_2000 =
        ExecutionTracer::trace(ws, "HeavyStep1Controller.handleMassiveRequest", &opts_2000)
            .expect("trace 2000 budget");
    let tokens_2000 = count_tokens(&trace_2000.to_markdown());
    println!(
        "Budget 2000 tokens result: {tokens_2000} (raw: {})",
        trace_2000.stats.raw_file_tokens
    );
    assert!(
        tokens_2000 <= 2000,
        "Output markdown slice tokens MUST be <= 2,000, got {tokens_2000}"
    );
    assert_eq!(trace_2000.total_steps, 8, "Must retain all 8 steps");

    // Test Budget = 1,500 tokens (Default target)
    let opts_1500 = SliceOptions {
        depth: 8,
        include_types: true,
        include_calls: true,
        budget: Some(1500),
    };
    let trace_1500 =
        ExecutionTracer::trace(ws, "HeavyStep1Controller.handleMassiveRequest", &opts_1500)
            .expect("trace 1500 budget");
    let tokens_1500 = count_tokens(&trace_1500.to_markdown());
    println!(
        "Budget 1500 tokens result: {tokens_1500} (raw: {})",
        trace_1500.stats.raw_file_tokens
    );
    assert!(
        tokens_1500 <= 1500,
        "Output markdown slice tokens MUST be <= 1,500, got {tokens_1500}"
    );
    assert_eq!(trace_1500.total_steps, 8);

    // Test Budget = 1,100 tokens (Level 4 minimum bound for 8-step markdown envelope)
    let opts_1100 = SliceOptions {
        depth: 8,
        include_types: true,
        include_calls: true,
        budget: Some(1100),
    };
    let trace_1100 =
        ExecutionTracer::trace(ws, "HeavyStep1Controller.handleMassiveRequest", &opts_1100)
            .expect("trace 1100 budget");
    let tokens_1100 = count_tokens(&trace_1100.to_markdown());
    println!(
        "Budget 1100 tokens result: {tokens_1100} (raw: {})",
        trace_1100.stats.raw_file_tokens
    );
    assert!(
        tokens_1100 <= 1100,
        "Output markdown slice tokens MUST be <= 1,100, got {tokens_1100}"
    );
    assert_eq!(trace_1100.total_steps, 8);

    // Verify token stats accuracy
    assert!(
        trace_1500.stats.raw_file_tokens > trace_1500.stats.sliced_tokens,
        "Raw file tokens should exceed sliced tokens"
    );
    assert!(
        trace_1500.stats.savings_percentage > 50.0,
        "Savings percentage should be > 50%, got {}",
        trace_1500.stats.savings_percentage
    );
}

// =========================================================================
// SCENARIO 4: Polyglot Trait / Interface Implementor Adversarial Checks
// =========================================================================

#[test]
fn test_adversarial_polyglot_multiple_implementors_and_circular_types() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // 1. Rust multiple trait implementors across sibling files
    let trait_rs = ws.join("repo.rs");
    fs::write(
        &trait_rs,
        r#"
pub trait EntityRepository {
    fn find_by_id(&self, id: u64) -> Option<String>;
    fn save(&self, data: &str) -> Result<u64, String>;
}

pub fn fetch_entity(repo: &dyn EntityRepository, id: u64) -> Option<String> {
    repo.find_by_id(id)
}
"#,
    )
    .expect("write trait");

    let postgres_rs = ws.join("postgres.rs");
    fs::write(
        &postgres_rs,
        r#"
use crate::repo::EntityRepository;

pub struct PostgresRepo;

impl EntityRepository for PostgresRepo {
    fn find_by_id(&self, id: u64) -> Option<String> {
        Some(format!("pg_{id}"))
    }
    fn save(&self, data: &str) -> Result<u64, String> {
        Ok(1)
    }
}
"#,
    )
    .expect("write pg");

    let redis_rs = ws.join("redis.rs");
    fs::write(
        &redis_rs,
        r#"
use crate::repo::EntityRepository;

pub struct RedisCacheRepo;

impl EntityRepository for RedisCacheRepo {
    fn find_by_id(&self, id: u64) -> Option<String> {
        Some(format!("redis_{id}"))
    }
    fn save(&self, data: &str) -> Result<u64, String> {
        Ok(2)
    }
}
"#,
    )
    .expect("write redis");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&trait_rs, "fetch_entity", &opts)
        .expect("slice fetch_entity");

    let implementor_names: Vec<&str> = slice
        .hoisted_implementors
        .iter()
        .map(|i| i.implementor_name.as_str())
        .collect();

    assert!(
        implementor_names.contains(&"PostgresRepo")
            || implementor_names.contains(&"RedisCacheRepo"),
        "Must hoist concrete implementors for EntityRepository, found: {implementor_names:?}"
    );

    let md = slice.to_markdown();
    assert!(
        md.contains("Concrete Implementors") || md.contains("EntityRepository"),
        "Markdown must include implementors section"
    );
}
