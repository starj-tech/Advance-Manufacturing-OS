import { assertEquals, assertThrows } from 'jsr:@std/assert';
import {
  buildReadRequest,
  parseReadResponse,
  parseRegisterAddress,
  ModbusError,
} from './modbus-tcp.ts';

Deno.test('buildReadRequest encodes MBAP + PDU correctly for holding read', () => {
  const buf = buildReadRequest(0x0042, 1, 'holding', 0x000a, 3);
  const view = new DataView(buf.buffer);
  assertEquals(view.getUint16(0), 0x0042); // tx id
  assertEquals(view.getUint16(2), 0); // protocol id
  assertEquals(view.getUint16(4), 6); // length
  assertEquals(buf[6], 1); // unit id
  assertEquals(buf[7], 0x03); // function code (holding)
  assertEquals(view.getUint16(8), 0x000a); // address
  assertEquals(view.getUint16(10), 3); // count
});

Deno.test('buildReadRequest uses 0x04 for input registers', () => {
  const buf = buildReadRequest(1, 2, 'input', 0, 1);
  assertEquals(buf[7], 0x04);
  assertEquals(buf[6], 2);
});

Deno.test('parseReadResponse extracts u16 registers from valid frame', () => {
  // tx=0x0042, unit=1, fc=0x03, byteCount=4, two registers 0x1234, 0xabcd
  const frame = new Uint8Array([
    0x00, 0x42, 0x00, 0x00, 0x00, 0x07, 0x01, 0x03, 0x04, 0x12, 0x34, 0xab, 0xcd,
  ]);
  const out = parseReadResponse(0x0042, 1, 0x03, frame);
  assertEquals(out, [0x1234, 0xabcd]);
});

Deno.test('parseReadResponse decodes Modbus exception', () => {
  // fc = 0x83 (0x03 | 0x80), exception code 0x02
  const frame = new Uint8Array([0x00, 0x42, 0x00, 0x00, 0x00, 0x03, 0x01, 0x83, 0x02]);
  const err = assertThrows(
    () => parseReadResponse(0x0042, 1, 0x03, frame),
    ModbusError,
  ) as ModbusError;
  assertEquals(err.code, 2);
});

Deno.test('parseReadResponse rejects tx id mismatch', () => {
  const frame = new Uint8Array([0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x03, 0x02, 0x00, 0x00]);
  assertThrows(() => parseReadResponse(0x0042, 1, 0x03, frame), ModbusError);
});

Deno.test('parseRegisterAddress handles Modicon notation', () => {
  assertEquals(parseRegisterAddress('40001'), { kind: 'holding', address: 0 });
  assertEquals(parseRegisterAddress('40010'), { kind: 'holding', address: 9 });
  assertEquals(parseRegisterAddress('30005'), { kind: 'input', address: 4 });
  assertEquals(parseRegisterAddress('5'), { kind: 'holding', address: 5 });
});
