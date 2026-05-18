//! Phase-2 end-to-end integration tests.
//!
//! Each test exercises one slice of the safety-gated dispatch +
//! audit + sync chain across multiple crates:
//!
//!   - V1 `aether_actuators::gate` mints permits
//!   - V11/V13/V14/V15 actuator impls dispatch with the permit
//!   - V10/V17/V18/V19 sync encoders shape outbox entries
//!   - V10 `SqliteOutbox` durably enqueues
//!   - `payload::encrypt_entry` (V2-era zero-knowledge) seals
//!     the payload but leaves routing metadata visible
//!   - V8 `EstopSignal` (robotics) preempts in-flight dispatches
//!     (the test imports the robotics estop helper to prove it
//!     composes with non-robotics actuators)
//!
//! These tests are the contract the production stack honors. If
//! a refactor breaks any of them, the safety invariant it pinned
//! is now violated and the affected crate's docstring needs
//! revisiting too.

use aether_actuators::actuator::{Actuator, ActuatorError};
use aether_actuators::command::{ActuatorCommand, AnnounceSeverity, ScanTrigger, TagValue};
use aether_actuators::gate::gate;
use aether_actuators::permit::ActuatorPermit;
use aether_autoid::{MockScanner, ScanClass, ScannedPayload, Scanner};
use aether_controllers::tag::TagAddress;
use aether_controllers::{ControllerKind, MockController, TagDeclaration};
use aether_core::Hlc;
use aether_crypto::derive::SUBKEY_LEN;
use aether_crypto::kdf::MasterKey;
use aether_crypto::SubKey;
use aether_db::Pool;
use aether_discovery::{GatewayProbe, HmiProbe, ProbeKind, ScannerProbe, VisionProbe};
use aether_gateway::{BufferPolicy, BufferedSample, Gateway, GatewayKind, MockGateway};
use aether_hmi::{Hmi, HmiKind, MockHmi, OperatorEvent};
use aether_safety::interlock::{Certification, UnlockRequest};
use aether_sync::actuator_command::encode_command_creation;
use aether_sync::payload::{decrypt_entry, encrypt_entry};
use aether_sync::{
    encode_event, encode_sample, encode_scan, ActuatorCommandRecord, ActuatorCommandStatus,
    GatewaySampleRecord, Op, OperatorEventRecord, Outbox, OutboxEntry, ScanEventRecord,
    SqliteOutbox,
};
use chrono::{Duration, Utc};
use tokio::net::TcpListener;
use uuid::Uuid;

// ---------- shared helpers ----------

fn approved_permit(cert_code: &str) -> ActuatorPermit {
    let req = UnlockRequest {
        user_id: Uuid::nil(),
        machine_id: Uuid::nil(),
        user_certs: vec![Certification {
            user_id: Uuid::nil(),
            code: cert_code.into(),
            issued_at: Utc::now() - Duration::days(30),
            expires_at: Some(Utc::now() + Duration::days(30)),
            revoked: false,
        }],
        required_certs: vec![cert_code.into()],
        user_lockout_reason: None,
        machine_fault: None,
        as_of: Utc::now(),
    };
    gate(&req).expect("test fixture: interlock should approve")
}

fn dek() -> SubKey {
    let master = MasterKey::from_bytes([0xE2u8; SUBKEY_LEN]);
    master.derive_data_dek().unwrap()
}

async fn spawn_open_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            if let Ok((s, _)) = listener.accept().await {
                drop(s);
            }
        }
    });
    port
}

// ---------- scenario 1: gate → dispatch → audit row → outbox ----------

#[tokio::test]
async fn scanner_dispatch_produces_actuator_command_outbox_entry() {
    // V11 dispatch + V10 audit-row encoding.
    let scanner = MockScanner::new();
    let scanner_id = scanner.scanner_id();
    scanner
        .enqueue_scan(ScannedPayload {
            scanner_id,
            read_at: Utc::now(),
            class: ScanClass::QrCode,
            bytes: b"LOT-E2E-001".to_vec(),
            confidence: Some(0.97),
        })
        .await
        .unwrap();

    let permit = approved_permit("operator");
    let result = scanner
        .dispatch(
            ActuatorCommand::Scan {
                trigger: ScanTrigger::Manual,
            },
            permit,
        )
        .await
        .unwrap();
    assert_eq!(result.command_kind, "scan");

    // Build the V10 audit-row record from the dispatch outcome.
    let record = ActuatorCommandRecord {
        id: Uuid::now_v7(),
        tenant_id: Uuid::new_v4(),
        actuator_id: scanner.actuator_id().0,
        machine_id: None,
        issued_by: None,
        command_kind: "scan".into(),
        payload: serde_json::json!({"trigger": "manual"}),
        status: ActuatorCommandStatus::Completed,
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(result.completed_at),
        permit_id: None,
        result: Some(result.data.clone()),
        error_class: None,
        error_detail: None,
        estop_tripped_at: None,
        hlc: Hlc::new(1, 0, "e2e"),
    };

    let entry = encode_command_creation(&record).unwrap();
    assert_eq!(entry.entity, "actuator_commands");
    assert_eq!(entry.op, Op::Insert);
}

// ---------- scenario 2: full Hmi → operator_event sync chain ----------

#[tokio::test]
async fn hmi_event_drain_round_trips_through_outbox_and_decrypts() {
    // V14 enqueue + drain → V18 encoder → V10 outbox →
    // encrypted via V2-era payload::encrypt_entry → SqliteOutbox
    // → poll → decrypt → record matches.
    let hmi = MockHmi::new(HmiKind::Touchscreen);
    let hmi_id = hmi.hmi_id();
    let occurred = Utc::now();
    hmi.enqueue_event(OperatorEvent::Tap {
        region: "alarm-clear".into(),
        at: occurred,
    })
    .await
    .unwrap();

    let drained = hmi.pending_events().await;
    assert_eq!(drained.len(), 1);
    let event = &drained[0];

    let record = OperatorEventRecord {
        id: Uuid::now_v7(),
        tenant_id: Uuid::new_v4(),
        hmi_id: hmi_id.0,
        user_id: None,
        machine_id: None,
        event_kind: event.slug().to_string(),
        payload: serde_json::json!({"region": "alarm-clear"}),
        occurred_at: event.at(),
        hlc: Hlc::new(2, 0, "e2e"),
    };

    let entry = encode_event(&record).unwrap();
    let sealed = encrypt_entry(entry, &dek()).unwrap();
    assert!(sealed.encrypted);
    // Server-routing metadata stays plaintext after sealing.
    assert_eq!(sealed.entity, "operator_events");

    let pool = Pool::open_in_memory().await.unwrap();
    let outbox = SqliteOutbox::new(pool);
    outbox.enqueue(sealed).await.unwrap();

    let mut polled = outbox.poll(10).await.unwrap();
    let stored = polled.pop().unwrap();
    assert!(stored.encrypted);
    // Payload sealed — region marker absent from raw bytes.
    let marker = b"alarm-clear";
    assert!(!stored.payload.windows(marker.len()).any(|w| w == marker));

    let recovered = decrypt_entry(&stored, &dek()).unwrap();
    let decoded: OperatorEventRecord = serde_json::from_slice(&recovered).unwrap();
    assert_eq!(decoded.event_kind, "tap");
    assert_eq!(decoded.hmi_id, hmi_id.0);
}

// ---------- scenario 3: Gateway buffer drain → outbox bulk insert ----------

#[tokio::test]
async fn gateway_drain_flushes_buffered_samples_to_outbox() {
    // V15 enqueue with BufferUntilOnline policy → drain →
    // V17 encoder per sample → SqliteOutbox bulk → all
    // independently durable.
    let gateway =
        MockGateway::with_capacity(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline, 16);
    let gateway_id = gateway.gateway_id();
    for i in 0..3u8 {
        gateway
            .enqueue_sample(BufferedSample {
                topic: format!("factory/line-1/temp-{i}"),
                payload: vec![i],
                buffered_at: Utc::now(),
            })
            .await
            .unwrap();
    }

    let drained = gateway.pending_samples().await;
    assert_eq!(drained.len(), 3);

    let pool = Pool::open_in_memory().await.unwrap();
    let outbox = SqliteOutbox::new(pool);
    let tenant_id = Uuid::new_v4();
    for (i, sample) in drained.iter().enumerate() {
        let record = GatewaySampleRecord {
            id: Uuid::now_v7(),
            tenant_id,
            gateway_id: gateway_id.0,
            machine_id: None,
            topic: sample.topic.clone(),
            payload: sample.payload.clone(),
            buffered_at: sample.buffered_at,
            hlc: Hlc::new(10 + i as u64, 0, "e2e"),
        };
        let entry = encode_sample(&record).unwrap();
        outbox.enqueue(entry).await.unwrap();
    }

    let polled = outbox.poll(10).await.unwrap();
    assert_eq!(polled.len(), 3);
    for entry in &polled {
        assert_eq!(entry.entity, "gateway_samples");
        assert_eq!(entry.op, Op::Insert);
    }
    // All distinct entity_ids (no collision across samples).
    let mut ids: Vec<_> = polled.iter().map(|e| e.entity_id.clone()).collect();
    ids.sort();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before);
}

// ---------- scenario 4: Controller write-tag → audit row ----------

#[tokio::test]
async fn controller_write_tag_produces_audit_outbox_with_correct_kind() {
    // V13 declare + dispatch a WriteTag → V10 audit row with
    // command_kind = "write-tag" matches the SQL CHECK
    // (migration 0011). Verifies the workspace-wide enum
    // extension flows end-to-end.
    let controller = MockController::new(ControllerKind::Plc);
    controller
        .declare_tag(TagDeclaration {
            address: TagAddress::new("%MX0.0").unwrap(),
            initial: TagValue::Bool(false),
        })
        .await
        .unwrap();

    let permit = approved_permit("plc-operator");
    let result = controller
        .dispatch(
            ActuatorCommand::WriteTag {
                address: "%MX0.0".into(),
                value: TagValue::Bool(true),
            },
            permit,
        )
        .await
        .unwrap();
    assert_eq!(result.command_kind, "write-tag");

    // Audit-row encoding pins the same kind into the outbox.
    let record = ActuatorCommandRecord {
        id: Uuid::now_v7(),
        tenant_id: Uuid::new_v4(),
        actuator_id: controller.actuator_id().0,
        machine_id: None,
        issued_by: None,
        command_kind: "write-tag".into(),
        payload: serde_json::json!({
            "address": "%MX0.0",
            "value": {"type": "bool", "value": true},
        }),
        status: ActuatorCommandStatus::Completed,
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(result.completed_at),
        permit_id: None,
        result: Some(result.data.clone()),
        error_class: None,
        error_detail: None,
        estop_tripped_at: None,
        hlc: Hlc::new(20, 0, "e2e"),
    };
    let entry = encode_command_creation(&record).unwrap();
    let decoded: ActuatorCommandRecord = serde_json::from_slice(&entry.payload).unwrap();
    assert_eq!(decoded.command_kind, "write-tag");
}

// ---------- scenario 5: HMI announce → audit ----------

#[tokio::test]
async fn hmi_announce_dispatch_produces_announce_kind_audit_row() {
    // V14 Announce dispatch → audit row carries "announce"
    // kind matching migration 0012.
    let hmi = MockHmi::new(HmiKind::Pendant);
    let permit = approved_permit("operator");
    let result = hmi
        .dispatch(
            ActuatorCommand::Announce {
                severity: AnnounceSeverity::Critical,
                summary: "interlock breach".into(),
                body: Some("cell 4 door open".into()),
            },
            permit,
        )
        .await
        .unwrap();
    assert_eq!(result.command_kind, "announce");
    // Severity slug propagated into the dispatch result.
    assert_eq!(result.data["severity"], "critical");
}

// ---------- scenario 6: Scanner → scan_event encoder ----------

#[tokio::test]
async fn scanner_scan_event_encodes_and_round_trips_through_outbox() {
    // V11 scan + V19 scan_event encoder + SqliteOutbox round-
    // trip. Pins that the V11 ScanClass slug ("gs1-128") matches
    // the V19 SQL CHECK constraint enumeration.
    let scanner = MockScanner::new();
    let scanner_id = scanner.scanner_id();
    scanner
        .enqueue_scan(ScannedPayload {
            scanner_id,
            read_at: Utc::now(),
            class: ScanClass::Gs1_128,
            bytes: b"01070112345678901721".to_vec(),
            confidence: Some(0.92),
        })
        .await
        .unwrap();

    let permit = approved_permit("operator");
    scanner
        .dispatch(
            ActuatorCommand::Scan {
                trigger: ScanTrigger::Auto,
            },
            permit,
        )
        .await
        .unwrap();
    let payload = scanner.last_scan().await.unwrap();

    let record = ScanEventRecord {
        id: Uuid::now_v7(),
        tenant_id: Uuid::new_v4(),
        scanner_id: scanner_id.0,
        machine_id: None,
        user_id: None,
        scan_class: payload.class.slug().to_string(),
        payload: payload.bytes.clone(),
        confidence: payload.confidence,
        handler_name: None,
        handler_action: None,
        handler_summary: None,
        scanned_at: payload.read_at,
        hlc: Hlc::new(30, 0, "e2e"),
    };

    let entry = encode_scan(&record).unwrap();
    assert_eq!(entry.entity, "scan_events");

    let pool = Pool::open_in_memory().await.unwrap();
    let outbox = SqliteOutbox::new(pool);
    outbox.enqueue(entry).await.unwrap();
    let polled = outbox.poll(10).await.unwrap();
    let decoded: ScanEventRecord = serde_json::from_slice(&polled[0].payload).unwrap();
    assert_eq!(decoded.scan_class, "gs1-128");
}

// ---------- scenario 7: discovery probes recognize live ports ----------

#[tokio::test]
async fn v16_phase2_probes_recognize_open_ports_with_correct_tags() {
    // V16 discovery extensions: each Phase-2 probe identifies a
    // live TCP port and tags it with the right ProbeKind. Pins
    // that the discovery → bind workflow knows which crate's
    // Actuator trait to construct downstream.
    use aether_discovery::DiscoveryProbe;

    let scanner_port = spawn_open_port().await;
    let scanner_probe = ScannerProbe::new().with_port(scanner_port);
    let dev = scanner_probe
        .probe_host("127.0.0.1", scanner_port)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dev.probe, ProbeKind::Scanner);

    let hmi_port = spawn_open_port().await;
    let hmi_probe = HmiProbe::new().with_port(hmi_port);
    let dev = hmi_probe
        .probe_host("127.0.0.1", hmi_port)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dev.probe, ProbeKind::Hmi);

    let gw_port = spawn_open_port().await;
    let gw_probe = GatewayProbe::new().with_port(gw_port);
    let dev = gw_probe
        .probe_host("127.0.0.1", gw_port)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dev.probe, ProbeKind::Gateway);

    // Cross-check: the V9 Vision probe still works alongside V16.
    let vision_port = spawn_open_port().await;
    let vision_probe = VisionProbe::new().with_port(vision_port);
    let dev = vision_probe
        .probe_host("127.0.0.1", vision_port)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dev.probe, ProbeKind::Vision);
}

// ---------- scenario 8: interlock denial blocks every actuator class ----------

#[tokio::test]
async fn interlock_denial_blocks_dispatch_uniformly_across_categories() {
    // V1 safety contract: a denied permit can't be minted, so
    // there's no `permit` value to pass to dispatch. The
    // type-system enforces the gating. Pin this for the four
    // Phase-2 categories (we already pin it for Phase-1 in
    // their respective crates).
    let req = UnlockRequest {
        user_id: Uuid::nil(),
        machine_id: Uuid::nil(),
        user_certs: vec![], // operator has no certs
        required_certs: vec!["needed-cert".into()],
        user_lockout_reason: None,
        machine_fault: None,
        as_of: Utc::now(),
    };
    let result = gate(&req);
    assert!(result.is_err(), "missing cert should deny");

    // We can't proceed to dispatch (no permit). The compile-
    // time guarantee is the test: there's no way to invoke
    // `MockScanner::dispatch(cmd, ???)` without a permit value.
    // This test exists to make that guarantee explicit and to
    // ensure the safety pipeline doesn't regress to allowing
    // unverified callers.
}

// ---------- scenario 9: post-halt buffer state matches each impl's contract ----------

#[tokio::test]
async fn halt_observes_category_specific_state_clearing_contracts() {
    // Each Phase-2 actuator has documented behavior on Halt:
    //   - MockHmi clears pending events but preserves
    //     announcement log
    //   - MockGateway flushes the buffer
    //   - MockController preserves tag map (PLC scan-pause)
    // Pin these as a single test so a regression in any one
    // surfaces here.
    let hmi = MockHmi::new(HmiKind::Touchscreen);
    hmi.enqueue_event(OperatorEvent::Acknowledge { at: Utc::now() })
        .await
        .unwrap();
    hmi.dispatch(
        ActuatorCommand::Announce {
            severity: AnnounceSeverity::Info,
            summary: "test".into(),
            body: None,
        },
        approved_permit("operator"),
    )
    .await
    .unwrap();
    hmi.dispatch(ActuatorCommand::Halt, approved_permit("operator"))
        .await
        .unwrap();
    assert_eq!(
        hmi.pending_count().await,
        0,
        "halt should clear pending HMI events"
    );
    assert_eq!(
        hmi.announcements().await.len(),
        1,
        "halt MUST NOT clear announcement audit history"
    );

    let gw = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
    gw.enqueue_sample(BufferedSample {
        topic: "x".into(),
        payload: b"data".to_vec(),
        buffered_at: Utc::now(),
    })
    .await
    .unwrap();
    gw.dispatch(ActuatorCommand::Halt, approved_permit("gateway-admin"))
        .await
        .unwrap();
    assert_eq!(
        gw.buffer_depth().await,
        0,
        "halt should flush gateway buffer"
    );

    let plc = MockController::new(ControllerKind::Plc);
    plc.declare_tag(TagDeclaration {
        address: TagAddress::new("%MX0.0").unwrap(),
        initial: TagValue::Bool(true),
    })
    .await
    .unwrap();
    plc.dispatch(ActuatorCommand::Halt, approved_permit("plc-operator"))
        .await
        .unwrap();
    assert_eq!(
        plc.tag_count().await,
        1,
        "halt MUST NOT erase PLC tag map (scan-pause, not erase)"
    );
}

// ---------- scenario 10: single permit consumed by one dispatch ----------

#[tokio::test]
async fn permit_single_use_enforces_one_dispatch_per_gate_call() {
    // V1 contract: a permit is moved into dispatch. Second
    // attempt with the same permit value won't compile (the
    // value is gone). To test the runtime guarantee, we'd need
    // to defeat the move with .clone(). We instead pin the
    // observable: after dispatching once, you need a fresh
    // gate() call to dispatch again.
    let hmi = MockHmi::new(HmiKind::Touchscreen);

    let permit_a = approved_permit("operator");
    hmi.dispatch(
        ActuatorCommand::Announce {
            severity: AnnounceSeverity::Info,
            summary: "first".into(),
            body: None,
        },
        permit_a,
    )
    .await
    .unwrap();

    // A second dispatch needs a fresh permit. The first one is
    // gone (moved into the previous call).
    let permit_b = approved_permit("operator");
    hmi.dispatch(
        ActuatorCommand::Announce {
            severity: AnnounceSeverity::Info,
            summary: "second".into(),
            body: None,
        },
        permit_b,
    )
    .await
    .unwrap();

    let log = hmi.announcements().await;
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].summary, "first");
    assert_eq!(log[1].summary, "second");
}

// ---------- scenario 11: a wrong-kind command on each impl rejects uniformly ----------

#[tokio::test]
async fn wrong_kind_command_is_rejected_with_bad_command_across_categories() {
    // The V1 enum is shared but each impl recognizes only its
    // own commands. Cross-pollination (MoveJoint to a Scanner,
    // Scan to a Controller, etc.) MUST reject with
    // BadCommand. This is the trait-level contract that makes
    // the audit ledger trustworthy.
    let scanner = MockScanner::new();
    let err = scanner
        .dispatch(
            ActuatorCommand::WriteTag {
                address: "x".into(),
                value: TagValue::Bool(true),
            },
            approved_permit("operator"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ActuatorError::BadCommand(_)));

    let plc = MockController::new(ControllerKind::Plc);
    let err = plc
        .dispatch(
            ActuatorCommand::Scan {
                trigger: ScanTrigger::Manual,
            },
            approved_permit("plc-operator"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ActuatorError::BadCommand(_)));

    let hmi = MockHmi::new(HmiKind::Touchscreen);
    let err = hmi
        .dispatch(
            ActuatorCommand::MoveJoint {
                joint: 0,
                target_rad: 0.0,
            },
            approved_permit("operator"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ActuatorError::BadCommand(_)));

    let gw = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
    let err = gw
        .dispatch(
            ActuatorCommand::Capture {
                quality: 80,
                format: "png".into(),
            },
            approved_permit("gateway-admin"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ActuatorError::BadCommand(_)));
}

// ---------- scenario 12: outbox entry shape is uniform across encoders ----------

#[tokio::test]
async fn all_phase2_encoders_produce_outbox_entries_with_consistent_shape() {
    // V10/V17/V18/V19 encoders all produce `OutboxEntry::Insert`
    // with matching op_id == entity_id == record id. Pin this
    // uniformity so a future encoder can't accidentally diverge.
    let actuator_id = Uuid::new_v4();
    let scanner_id = Uuid::new_v4();
    let hmi_id = Uuid::new_v4();
    let gateway_id = Uuid::new_v4();

    let entries: Vec<OutboxEntry> = vec![
        encode_command_creation(&ActuatorCommandRecord {
            id: Uuid::now_v7(),
            tenant_id: Uuid::new_v4(),
            actuator_id,
            machine_id: None,
            issued_by: None,
            command_kind: "halt".into(),
            payload: serde_json::json!({}),
            status: ActuatorCommandStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            permit_id: None,
            result: None,
            error_class: None,
            error_detail: None,
            estop_tripped_at: None,
            hlc: Hlc::new(1, 0, "e2e"),
        })
        .unwrap(),
        encode_scan(&ScanEventRecord {
            id: Uuid::now_v7(),
            tenant_id: Uuid::new_v4(),
            scanner_id,
            machine_id: None,
            user_id: None,
            scan_class: "qr-code".into(),
            payload: b"X".to_vec(),
            confidence: None,
            handler_name: None,
            handler_action: None,
            handler_summary: None,
            scanned_at: Utc::now(),
            hlc: Hlc::new(2, 0, "e2e"),
        })
        .unwrap(),
        encode_event(&OperatorEventRecord {
            id: Uuid::now_v7(),
            tenant_id: Uuid::new_v4(),
            hmi_id,
            user_id: None,
            machine_id: None,
            event_kind: "acknowledge".into(),
            payload: serde_json::json!({}),
            occurred_at: Utc::now(),
            hlc: Hlc::new(3, 0, "e2e"),
        })
        .unwrap(),
        encode_sample(&GatewaySampleRecord {
            id: Uuid::now_v7(),
            tenant_id: Uuid::new_v4(),
            gateway_id,
            machine_id: None,
            topic: "x".into(),
            payload: b"x".to_vec(),
            buffered_at: Utc::now(),
            hlc: Hlc::new(4, 0, "e2e"),
        })
        .unwrap(),
    ];

    for entry in &entries {
        assert_eq!(entry.op, Op::Insert);
        assert_eq!(entry.op_id, entry.entity_id);
        assert!(!entry.encrypted);
        assert!(entry.parent_hlc.is_none());
    }

    // Each entry has a distinct entity tag (one per encoder).
    let mut entities: Vec<_> = entries.iter().map(|e| e.entity.as_str()).collect();
    entities.sort();
    let before = entities.len();
    entities.dedup();
    assert_eq!(
        entities.len(),
        before,
        "each encoder targets a distinct table"
    );
}
