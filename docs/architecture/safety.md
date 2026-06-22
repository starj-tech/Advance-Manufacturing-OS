# Safety Subsystem (Employee Shell)

> Status: skeleton. Multi-tier broadcast + geofence evaluator land in PR #5.

## SOS pipeline

Multi-tier; designed to function offline.

```
Tier 0: Hardware
   wearable button (BLE) or in-app long-press

Tier 1: Local LAN broadcast
   mDNS UDP multicast — every Aether instance on the LAN gets the alert.
   Works with no infrastructure beyond Wi-Fi.

Tier 2: MQTT broker (factory backbone)
   topic: aether/sos/{site}/{user}, QoS 2, retained.

Tier 3: Supabase Realtime
   INSERT INTO sos_events; broadcast to subscribed managers.
   Authoritative for audit.

Tier 4: External escalation
   Edge Function sos-escalation fans out to SMS/Slack/email/webhook.
```

Each event has a UUIDv7 `event_id`. Tier 3 deduplicates. Lower tiers
fire even when WAN is down.

Implementation: `crates/aether-safety/src/sos.rs`. Skeleton emits
`NotImplemented`; PR #5 wires `mdns-sd`, `rumqttc`, and Supabase REST.

## Geofencing

Pure GPS fails indoors. Pure Wi-Fi BSSID is spoofable. Combine:

```
GeofenceEvaluator:
  evidence = []
  if gps_accuracy < 30m:
    evidence.push(Gps { inside, accuracy })
  if wifi_scan_available:
    fraction = matched_bssids / fence.allowed_bssids.len()
    evidence.push(WifiBssid { fraction })
  if ble_beacons_seen:
    fraction = matched_beacons / fence.allowed_beacons.len()
    evidence.push(Bluetooth { fraction })

  weighted vote: GPS=0.5, Wi-Fi=0.3, BLE=0.2
  threshold: 0.6
```

Implementation in `crates/aether-safety/src/geofence.rs` with unit tests.

False-positive mitigation: a 30s dwell-outside-zone window before
escalation, instead of immediate lockout.

## Glove-Friendly UI guidelines

`packages/glove-kit` exports tokens enforcing these:

| Aspect              | Spec                                                     |
| ------------------- | -------------------------------------------------------- |
| Touch target        | min 56dp (vs Apple HIG 44dp — gloves reduce precision)   |
| Tap padding         | min 12dp around interactive elements                     |
| Body type           | min 16px                                                 |
| Primary action type | min 20px                                                 |
| Contrast            | WCAG AAA (≥ 7:1)                                         |
| Color               | not the only encoder of state (color-blind safe)         |
| Gesture             | tap / press-and-hold; no multi-touch swipe               |
| Feedback            | visual + haptic + audible (triple-redundant)             |
| Layout              | vertical-first, single column, primary buttons at bottom |
| Irreversible action | explicit confirm ("Are you sure?")                       |

The `SosButton` component implements the press-and-hold pattern (default
2s) with a confirmation halo to avoid accidental triggers.
