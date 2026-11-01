#!/usr/bin/env python3
"""Capture malformed recipe-book STREAM inputs through unchanged original aee."""
import base64
import hashlib
import json

from server_recipe_book_generate import Original, ROOT, SHA, atomic_json


def queries():
    out = []

    def case(name, payload):
        out.append(dict(name=name, operations=[dict(op='packet_decode', wire=base64.b64encode(payload).decode())]))

    flags = bytes(8)
    init = b'\x00' + flags
    item = b'minecraft:stone'
    alias_zero = bytes([0x80, 0x80, 0x80, 0x80, 0x10])
    case('canonical/init-empty', init + b'\x00\x00')
    for state in range(3):
        case(f'state/fifth-high-bits-alias-{state}', bytes([0x80 | state, 0x80, 0x80, 0x80, 0x10]) + flags + bytes(2 if state == 0 else 1))
    for high in [0x20, 0x30, 0x70]:
        case(f'state/fifth-high-bits-{high:x}', bytes([0x80, 0x80, 0x80, 0x80, high]) + flags + b'\x00\x00')
    case('state/sixth-byte-overflow', bytes([0x80] * 5 + [0]) + flags + b'\x00\x00')
    case('state/five-continuation-eof', bytes([0x80] * 5))
    case('state/negative-decoded', bytes([0xff, 0xff, 0xff, 0xff, 0x0f]) + flags + b'\x00\x00')
    case('state/nonexistent-three', b'\x03' + flags + b'\x00\x00')
    case('state/eof-empty', b'')
    case('state/eof-continuation', b'\x80')
    case('settings/eof-after-seven', b'\x00' + bytes(7))
    case('recipes/count-alias-zero', init + alias_zero + b'\x00')
    case('recipes/count-alias-one', init + bytes([0x81, 0x80, 0x80, 0x80, 0x10]) + bytes([len(item)]) + item + b'\x00')
    case('recipes/count-sixth-byte-overflow', init + bytes([0x80] * 5 + [0]) + b'\x00')
    case('recipes/count-negative', init + bytes([0xff, 0xff, 0xff, 0xff, 0x0f]) + b'\x00')
    case('recipes/count-eof', init)
    case('highlights/count-alias-zero', init + b'\x00' + alias_zero)
    case('highlights/count-alias-one', init + b'\x00' + bytes([0x81, 0x80, 0x80, 0x80, 0x10]) + bytes([len(item)]) + item)
    case('highlights/count-sixth-byte-overflow', init + b'\x00' + bytes([0x80] * 5 + [0]))
    case('highlights/count-negative', init + b'\x00' + bytes([0xff, 0xff, 0xff, 0xff, 0x0f]))
    case('highlights/count-eof', init + b'\x00')
    case('string/length-alias-zero', init + b'\x01' + alias_zero + b'\x00')
    case('string/length-alias-fifteen', init + b'\x01' + bytes([0x80 | len(item), 0x80, 0x80, 0x80, 0x10]) + item + b'\x00')
    case('string/length-sixth-byte-overflow', init + b'\x01' + bytes([0x80] * 5 + [0]))
    case('string/length-negative', init + b'\x01' + bytes([0xff, 0xff, 0xff, 0xff, 0x0f]))
    case('string/length-eof', init + b'\x01')
    case('string/content-eof', init + b'\x01' + bytes([len(item)]) + item[:-1])
    case('payload/trailing-byte', init + b'\x00\x00\x55')
    return out


def main():
    oracle = Original()
    inputs = queries()
    actual = oracle.run(inputs)
    fixture = dict(minecraft='1.21', oracle_inner_sha256=SHA,
                   scope='Actual unchanged aee.STREAM_CODEC.decode plus original encode on the decoded value. Fifth-byte unused VarInt high bits, overflow, negative values, and EOF are observed, not inferred. Payloads exclude packet ID and framing. Errors are recorded as actual Java exception classes/messages; these inputs do not run gameplay.',
                   cases=[dict(query=q, expected=row) for q, row in zip(inputs, actual)])
    path = ROOT / 'fixtures/server-recipe-book-codec-edges.json'
    atomic_json(path, fixture)
    print(json.dumps(dict(ok=True, cases=len(inputs), errors=sum(s['error'] is not None for row in actual for s in row['states']),
                          fixture_sha256=hashlib.sha256(path.read_bytes()).hexdigest())))


if __name__ == '__main__':
    main()
