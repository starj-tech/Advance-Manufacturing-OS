# IoT Auto-Discovery (Zero-Config)

> Status: skeleton. Real probe implementations land in PR #3 alongside
> the OPC-UA / MQTT bridges.

The promise: a manager opens AETHER-OS, hits **Scan network**, and within
minutes sees every PLC, sensor gateway, and protocol adapter on the
factory LAN — with suggested tag bindings ready to accept.

## Probes

`crates/aether-discovery/src/probe.rs` defines the `DiscoveryProbe` trait
and four implementations:

| Probe       | Default port | Method                                               |
| ----------- | ------------ | ---------------------------------------------------- |
| OPC-UA      | 4840         | `find_servers` discovery service                     |
| MQTT        | 1883         | clean CONNECT/DISCONNECT                             |
| Modbus      | 502          | function code 0x2B/0x0E "Read Device Identification" |
| EtherNet/IP | 44818        | CIP List Identity (UDP)                              |

Adding a new protocol is a matter of implementing the trait — the
scanner picks it up via dependency injection.

## Scanner

`Scanner::scan(req)`:

1. Resolve `req.cidr` to a host iterator (or detect default-gateway subnet).
2. For each (host × probe), dispatch under a semaphore (`req.concurrency`
   default 64).
3. Deduplicate by fingerprint (MAC where available, host:port hash otherwise).
4. Apply vendor templates → emit `SuggestedBinding` hints with confidence levels.

## Trust boundary

Discovery is **read-only**. Probes never write to a PLC. Saved devices
land in `machines` via the user's explicit "Add to inventory" action,
audited in `audit_log`.

## Verification

- Mock OPC-UA server (`tools/opcua-mock-server`) gets discovered in CI.
- Property tests (PR #3): random CIDR + probe set converges on
  deterministic device list.
