//! Integration tests for Milestone 1 Feature 3: Polyglot Interface & Trait Implementor Hoisting.

use ctxcut_core::model::SliceOptions;
use ctxcut_core::slice::ContextSlicer;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_hoist_rust_trait_implementors() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let trait_file = ws.join("service.rs");
    fs::write(
        &trait_file,
        r"
pub trait PaymentGateway {
    fn process_payment(&self, amount: u64) -> Result<String, String>;
}

pub fn execute_charge(gateway: &dyn PaymentGateway, amount: u64) -> Result<String, String> {
    gateway.process_payment(amount)
}
",
    )
    .expect("write trait");

    let impl_file = ws.join("stripe.rs");
    fs::write(
        &impl_file,
        r#"
use crate::service::PaymentGateway;

pub struct StripeGateway {
    pub api_key: String,
}

impl PaymentGateway for StripeGateway {
    fn process_payment(&self, amount: u64) -> Result<String, String> {
        Ok(format!("Charged ${amount} via Stripe"))
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

    let slice = slicer
        .slice_symbol(&trait_file, "execute_charge", &opts)
        .expect("slice symbol");

    assert!(!slice.hoisted_implementors.is_empty());
    let imp = &slice.hoisted_implementors[0];
    assert_eq!(imp.interface_name, "PaymentGateway");
    assert_eq!(imp.implementor_name, "StripeGateway");
    assert_eq!(imp.kind, "rust_impl");
    assert!(imp.definition.contains("impl PaymentGateway for StripeGateway"));

    let md = slice.to_markdown();
    assert!(md.contains("#### 3. Concrete Implementors"));
    assert!(md.contains("impl PaymentGateway for StripeGateway"));
}

#[test]
fn test_hoist_go_interface_implementors() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let iface_file = ws.join("gateway.go");
    fs::write(
        &iface_file,
        r"
package payment

type Processor interface {
    Charge(amount int) (string, error)
}

func ExecutePayment(p Processor, amount int) (string, error) {
    return p.Charge(amount)
}
",
    )
    .expect("write iface");

    let impl_file = ws.join("paypal.go");
    fs::write(
        &impl_file,
        r#"
package payment

type PaypalProcessor struct {
    ClientId string
}

func (p *PaypalProcessor) Charge(amount int) (string, error) {
    return "paypal_tx_123", nil
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

    let slice = slicer
        .slice_symbol(&iface_file, "ExecutePayment", &opts)
        .expect("slice go");

    assert!(!slice.hoisted_implementors.is_empty());
    let imp = &slice.hoisted_implementors[0];
    assert_eq!(imp.interface_name, "Processor");
    assert_eq!(imp.implementor_name, "PaypalProcessor");
    assert_eq!(imp.kind, "go_struct");
    assert!(imp.definition.contains("PaypalProcessor"));
}

#[test]
fn test_hoist_typescript_implements_clause() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let iface_file = ws.join("notifier.ts");
    fs::write(
        &iface_file,
        r#"
export interface NotificationService {
    send(recipient: string, message: string): Promise<boolean>;
}

export async function broadcastAlert(notifier: NotificationService, msg: string) {
    return await notifier.send("all", msg);
}
"#,
    )
    .expect("write iface");

    let impl_file = ws.join("slack.ts");
    fs::write(
        &impl_file,
        r"
import { NotificationService } from './notifier';

export class SlackNotificationService implements NotificationService {
    public async send(recipient: string, message: string): Promise<boolean> {
        // Send webhook
        return true;
    }
}
",
    )
    .expect("write impl");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&iface_file, "broadcastAlert", &opts)
        .expect("slice ts");

    assert!(!slice.hoisted_implementors.is_empty());
    let imp = &slice.hoisted_implementors[0];
    assert_eq!(imp.interface_name, "NotificationService");
    assert_eq!(imp.implementor_name, "SlackNotificationService");
    assert_eq!(imp.kind, "ts_class");
    assert!(imp.definition.contains("class SlackNotificationService implements NotificationService"));
}

#[test]
fn test_hoist_python_protocol_implementors() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();

    let proto_file = ws.join("protocol.py");
    fs::write(
        &proto_file,
        r"
from typing import Protocol

class DataSink(Protocol):
    def flush_records(self, records: list) -> int:
        ...

def sync_data(sink: DataSink, items: list) -> int:
    return sink.flush_records(items)
",
    )
    .expect("write proto");

    let impl_file = ws.join("s3_sink.py");
    fs::write(
        &impl_file,
        r"
from protocol import DataSink

class S3DataSink(DataSink):
    def flush_records(self, records: list) -> int:
        # S3 multipart upload
        return len(records)
",
    )
    .expect("write sink");

    let slicer = ContextSlicer::new();
    let opts = SliceOptions {
        depth: 2,
        include_types: true,
        include_calls: true,
        budget: None,
    };

    let slice = slicer
        .slice_symbol(&proto_file, "sync_data", &opts)
        .expect("slice python");

    assert!(!slice.hoisted_implementors.is_empty());
    let imp = &slice.hoisted_implementors[0];
    assert_eq!(imp.interface_name, "DataSink");
    assert_eq!(imp.implementor_name, "S3DataSink");
    assert_eq!(imp.kind, "py_class");
    assert!(imp.definition.contains("class S3DataSink(DataSink)"));
}
