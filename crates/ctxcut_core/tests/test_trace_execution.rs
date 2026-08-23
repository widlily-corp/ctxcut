//! Integration tests for Milestone 1 Feature 2: End-to-End Execution Trace Slicing.

use ctxcut_core::model::SliceOptions;
use ctxcut_core::resolver::ExecutionTracer;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_execution_trace_end_to_end_route_to_db() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    // 1. Controller / Route handler file
    let route_file = ws.join("order_route.ts");
    fs::write(
        &route_file,
        r"
import { OrderService } from './order_service';

export class OrderController {
    public static async createOrderHandler(req: any, res: any) {
        const order = req.body;
        const service = new OrderService();
        const result = await service.processOrder(order);
        res.json(result);
    }
}
",
    )
    .expect("write route");

    // 2. Service file
    let service_file = ws.join("order_service.ts");
    fs::write(
        &service_file,
        r"
import { OrderRepository } from './order_repo';

export class OrderService {
    public async processOrder(order: any) {
        const repo = new OrderRepository();
        return await repo.saveOrder(order);
    }
}
",
    )
    .expect("write service");

    // 3. Repository / DB sink file
    let repo_file = ws.join("order_repo.ts");
    fs::write(
        &repo_file,
        r#"
export class OrderRepository {
    public async saveOrder(order: any) {
        // DB insert
        return { id: "ord_123", status: "created", ...order };
    }
}
"#,
    )
    .expect("write repo");

    let opts = SliceOptions {
        depth: 5,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = ExecutionTracer::trace(ws, "OrderController.createOrderHandler", &opts)
        .expect("trace execution");

    assert_eq!(result.total_steps, 3);
    assert_eq!(result.steps[0].symbol_name, "OrderController.createOrderHandler");
    assert_eq!(result.steps[0].step_number, 1);
    assert_eq!(result.steps[1].symbol_name, "OrderService.processOrder");
    assert_eq!(result.steps[1].step_number, 2);
    assert_eq!(result.steps[2].symbol_name, "OrderRepository.saveOrder");
    assert_eq!(result.steps[2].step_number, 3);

    // Verify markdown trace formatting
    let md = result.to_markdown();
    assert!(md.contains("# Execution Flow Trace: `OrderController.createOrderHandler`"));
    assert!(md.contains("### Step 1: `OrderController.createOrderHandler`"));
    assert!(md.contains("### Step 2: `OrderService.processOrder`"));
    assert!(md.contains("### Step 3: `OrderRepository.saveOrder`"));
}

#[test]
fn test_execution_trace_cycle_protection() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let cyclic_file = ws.join("cycle.ts");
    fs::write(
        &cyclic_file,
        r"
export class LoopA {
    public static stepA() {
        return LoopB.stepB();
    }
}

export class LoopB {
    public static stepB() {
        return LoopA.stepA();
    }
}
",
    )
    .expect("write cycle");

    let opts = SliceOptions {
        depth: 8,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let result = ExecutionTracer::trace(ws, "LoopA.stepA", &opts).expect("trace cycle");

    // Traversal terminates when cycle is detected
    assert_eq!(result.total_steps, 2);
    let last_step = &result.steps[1];
    assert!(last_step.next_target.as_ref().unwrap().contains("cycle detected"));
}

#[test]
fn test_execution_trace_budget_compression() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let controller_file = ws.join("controller.ts");
    fs::write(
        &controller_file,
        r"
import { Service } from './service';

export function handleRequest() {
    // Very verbose comment line 1
    // Very verbose comment line 2
    // Very verbose comment line 3
    return Service.execute();
}
",
    )
    .expect("write controller");

    let service_file = ws.join("service.ts");
    fs::write(
        &service_file,
        r"
import { Repo } from './repo';

export class Service {
    public static execute() {
        // Detailed logging and business checks
        // Another comment block
        return Repo.save();
    }
}
",
    )
    .expect("write service");

    let repo_file = ws.join("repo.ts");
    fs::write(
        &repo_file,
        r"
export class Repo {
    public static save() {
        // DB commit
        return true;
    }
}
",
    )
    .expect("write repo");

    let opts = SliceOptions {
        depth: 5,
        include_types: true,
        include_calls: true,
        budget: Some(250), // Strict budget forcing folding
    };

    let result = ExecutionTracer::trace(ws, "handleRequest", &opts).expect("trace budget");
    println!("DEBUG sliced_tokens: {}, raw_tokens: {}\nMD:\n{}", result.stats.sliced_tokens, result.stats.raw_file_tokens, result.to_markdown());
    assert_eq!(result.total_steps, 3);
    assert!(result.stats.sliced_tokens <= 500);
}
