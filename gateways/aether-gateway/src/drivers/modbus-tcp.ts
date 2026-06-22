/**
 * Modbus TCP client — hand-rolled because Deno doesn't have a battle-tested
 * Modbus npm/jsr package and we want the gateway binary to stay small. Only
 * the read paths needed by the demo: function codes 0x03 (holding registers)
 * and 0x04 (input registers).
 *
 * Wire format (MBAP + PDU):
 *   transactionId (2 bytes) | protocolId (2 bytes, always 0)
 *   length (2 bytes)        | unitId (1 byte)
 *   functionCode (1 byte)   | data ...
 */

export type RegisterKind = 'holding' | 'input';

const FUNCTION_CODE: Record<RegisterKind, number> = {
  holding: 0x03,
  input: 0x04,
};

export class ModbusError extends Error {
  constructor(
    public readonly code: number,
    msg: string,
  ) {
    super(msg);
  }
}

export interface ModbusConnection {
  read: (kind: RegisterKind, address: number, count: number) => Promise<number[]>;
  close: () => void;
}

/** Build the MBAP+PDU request bytes for a register read. */
export function buildReadRequest(
  txId: number,
  unitId: number,
  kind: RegisterKind,
  address: number,
  count: number,
): Uint8Array {
  const buf = new Uint8Array(12);
  const view = new DataView(buf.buffer);
  view.setUint16(0, txId & 0xffff);
  view.setUint16(2, 0); // protocol id
  view.setUint16(4, 6); // length (remaining bytes after this field)
  buf[6] = unitId & 0xff;
  buf[7] = FUNCTION_CODE[kind];
  view.setUint16(8, address & 0xffff);
  view.setUint16(10, count & 0xffff);
  return buf;
}

/** Parse a Modbus read-response PDU's data section into u16 registers. */
export function parseReadResponse(
  expectedTxId: number,
  expectedUnitId: number,
  expectedFc: number,
  resp: Uint8Array,
): number[] {
  if (resp.length < 9) {
    throw new ModbusError(0, `short response (${resp.length} bytes)`);
  }
  const view = new DataView(resp.buffer, resp.byteOffset, resp.byteLength);
  const txId = view.getUint16(0);
  if (txId !== expectedTxId) {
    throw new ModbusError(0, `tx id mismatch: ${txId} vs ${expectedTxId}`);
  }
  const unitId = resp[6];
  if (unitId !== expectedUnitId) {
    throw new ModbusError(0, `unit id mismatch: ${unitId} vs ${expectedUnitId}`);
  }
  const fc = resp[7];
  // Exception path: function code OR 0x80, exception code in next byte.
  if (fc === (expectedFc | 0x80)) {
    const ex = resp[8];
    throw new ModbusError(ex, `modbus exception ${ex}`);
  }
  if (fc !== expectedFc) {
    throw new ModbusError(0, `function code mismatch: ${fc} vs ${expectedFc}`);
  }
  const byteCount = resp[8];
  if (resp.length < 9 + byteCount) {
    throw new ModbusError(0, `truncated payload`);
  }
  const out: number[] = [];
  for (let i = 0; i < byteCount; i += 2) {
    out.push(view.getUint16(9 + i));
  }
  return out;
}

/** Parse an "address" like "40001" (Modicon convention) or just "1" into
 *  (kind, zeroBasedAddress). Holding registers are 4xxxx, input are 3xxxx. */
export function parseRegisterAddress(input: string): { kind: RegisterKind; address: number } {
  const n = Number(input);
  if (!Number.isFinite(n) || n < 0) throw new Error(`invalid register address: ${input}`);
  if (n >= 30001 && n < 40000) return { kind: 'input', address: n - 30001 };
  if (n >= 40001) return { kind: 'holding', address: n - 40001 };
  // 0-based assumed holding when no Modicon prefix.
  return { kind: 'holding', address: n };
}

/** Open a Modbus TCP connection. Caller is responsible for calling close(). */
export async function connect(opts: {
  host: string;
  port: number;
  unitId: number;
  timeoutMs?: number;
}): Promise<ModbusConnection> {
  const conn = await Deno.connect({ hostname: opts.host, port: opts.port });
  let txCounter = 1;
  const timeout = opts.timeoutMs ?? 3_000;
  let buffer = new Uint8Array(0);

  async function readFromSocket(): Promise<void> {
    const chunk = new Uint8Array(1024);
    const n = await conn.read(chunk);
    if (n === null) throw new ModbusError(0, 'connection closed');
    const merged = new Uint8Array(buffer.length + n);
    merged.set(buffer, 0);
    merged.set(chunk.subarray(0, n), buffer.length);
    buffer = merged;
  }

  async function readResponse(): Promise<Uint8Array> {
    const start = Date.now();
    while (buffer.length < 8) {
      if (Date.now() - start > timeout) throw new ModbusError(0, 'read timeout (header)');
      await readFromSocket();
    }
    const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
    const length = view.getUint16(4);
    const total = 6 + length;
    while (buffer.length < total) {
      if (Date.now() - start > timeout) throw new ModbusError(0, 'read timeout (body)');
      await readFromSocket();
    }
    const out = buffer.slice(0, total);
    buffer = buffer.slice(total);
    return out;
  }

  return {
    async read(kind, address, count) {
      const txId = txCounter++ & 0xffff;
      const req = buildReadRequest(txId, opts.unitId, kind, address, count);
      await conn.write(req);
      const resp = await readResponse();
      return parseReadResponse(txId, opts.unitId, FUNCTION_CODE[kind], resp);
    },
    close() {
      try {
        conn.close();
      } catch {
        /* ignore */
      }
    },
  };
}
