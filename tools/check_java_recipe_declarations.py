#!/usr/bin/env python3
"""Capture original 1.21 UpdateRecipes during a normal, no-action offline login.

Only the copied world's server properties change. No recipe/tag bootstrap,
agent, transformed class, server command, or reconstructed declaration is used.
The original server launches and the raw socket client runs in this process tree.
"""
from pathlib import Path
import argparse
import base64
import hashlib
import json
import os
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import time
import uuid
import zlib

ROOT = Path(__file__).resolve().parents[1]
INNER_SHA256 = '8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01'
OUTER_SHA1 = '450698d1863ab5180c25d7c804ef0fe6369dd1ba'
JAVA = '/root/.local/share/mc-jdk21/jdk-21.0.13.0.2+1/bin/java'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(directory):
    return {str(p.relative_to(directory)): digest(p)
            for p in sorted(directory.rglob('*')) if p.is_file()}


def snapshot_digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def varint(value):
    out = bytearray()
    while True:
        byte = value & 127
        value >>= 7
        out.append(byte | (128 if value else 0))
        if not value:
            return bytes(out)


def read_varint(data, offset=0):
    value = 0
    for index in range(5):
        if offset + index >= len(data):
            raise ValueError('truncated VarInt')
        byte = data[offset + index]
        value |= (byte & 127) << (7 * index)
        if not byte & 128:
            return value, offset + index + 1
    raise ValueError('oversized VarInt')


def string(value):
    raw = value.encode('utf-8')
    return varint(len(raw)) + raw


def receive_exact(connection, count):
    result = bytearray()
    while len(result) < count:
        chunk = connection.recv(count - len(result))
        if not chunk:
            raise EOFError('server disconnected during login')
        result.extend(chunk)
    return bytes(result)


def receive_frame(connection, compression):
    length_bytes = bytearray()
    for _ in range(5):
        length_bytes.extend(receive_exact(connection, 1))
        if not length_bytes[-1] & 128:
            break
    length, end = read_varint(length_bytes)
    assert end == len(length_bytes) and 0 < length <= 8 * 1024 * 1024
    framed = receive_exact(connection, length)
    if compression is None:
        return framed, bytes(length_bytes) + framed
    uncompressed_length, offset = read_varint(framed)
    if uncompressed_length:
        assert uncompressed_length >= compression
        decompressor = zlib.decompressobj()
        packet = decompressor.decompress(framed[offset:]) + decompressor.flush()
        assert decompressor.eof and not decompressor.unused_data and not decompressor.unconsumed_tail
        assert len(packet) == uncompressed_length
    else:
        packet = framed[offset:]
        assert len(packet) < compression
    return packet, bytes(length_bytes) + framed


def send_packet(connection, packet_id, body=b'', compression=None):
    packet = varint(packet_id) + body
    if compression is not None:
        packet = (varint(len(packet)) + zlib.compress(packet) if len(packet) >= compression
                  else b'\0' + packet)
    connection.sendall(varint(len(packet)) + packet)


def capture(port):
    """Ignore all recipe internals; only framing/state control and count are read."""
    username = 'RecipeCapture'
    offline_uuid = bytearray(hashlib.md5(('OfflinePlayer:' + username).encode()).digest())
    offline_uuid[6] = offline_uuid[6] & 15 | 48
    offline_uuid[8] = offline_uuid[8] & 63 | 128
    state, compression = 'login', None
    observed, sent = [], []
    deadline = time.monotonic() + 60
    with socket.create_connection(('127.0.0.1', port), timeout=10) as connection:
        connection.settimeout(10)
        send_packet(connection, 0, varint(767) + string('localhost') + struct.pack('>H', port) + varint(2))
        send_packet(connection, 0, string(username) + offline_uuid)
        sent += [{'state': 'handshake', 'packet_id': 0}, {'state': 'login', 'packet_id': 0}]

        def send(packet_id, body=b''):
            send_packet(connection, packet_id, body, compression)
            sent.append({'state': state, 'packet_id': packet_id})

        while time.monotonic() < deadline:
            packet, wire_frame = receive_frame(connection, compression)
            packet_id, offset = read_varint(packet)
            body = packet[offset:]
            observed.append({'state': state, 'packet_id': packet_id, 'packet_bytes': len(packet)})
            if state == 'login':
                if packet_id == 0:
                    raise RuntimeError('login disconnect: ' + body.hex())
                if packet_id == 1:
                    raise RuntimeError('unexpected encryption request; offline-mode required')
                if packet_id == 3:
                    compression, end = read_varint(body)
                    assert end == len(body) and compression > 0
                elif packet_id == 2:
                    send(3)  # ServerboundLoginAcknowledgedPacket.
                    state = 'configuration'
                    # Normal Client Information; no gameplay action or movement.
                    send(0, string('en_us') + b'\x02' + varint(0) + b'\x01\x7f' + varint(1) + b'\0\x01')
                elif packet_id == 4:
                    transaction, _ = read_varint(body)
                    send(2, varint(transaction) + b'\0')
                elif packet_id == 5:
                    send(4, body + b'\0')
            elif state == 'configuration':
                if packet_id == 2:
                    raise RuntimeError('configuration disconnect: ' + body.hex())
                if packet_id == 3:
                    send(3)  # ServerboundFinishConfigurationPacket.
                    state = 'play'
                elif packet_id in (4, 5):
                    send(packet_id, body)  # Keep alive or ping/pong.
                elif packet_id == 14:
                    send(7, varint(0))  # Normal response: no locally known packs.
                elif packet_id == 0:
                    send(1, body + b'\0')  # Cookie absent.
            elif state == 'play':
                if packet_id == 0x1d:
                    raise RuntimeError('play disconnect: ' + body.hex())
                if packet_id == 0x26:
                    send(0x18, body)
                elif packet_id == 0x35:
                    send(0x27, body)
                elif packet_id == 0x77:
                    count, _ = read_varint(body)
                    assert count == 1290, f'unexpected recipe count {count}'
                    assert any(p['state'] == 'play' and p['packet_id'] == 0x2b for p in observed)
                    return {
                        'minecraft': '1.21', 'protocol': 767, 'packet_id': packet_id,
                        'packet_id_hex': '0x77', 'packet_id_varint_hex': packet[:offset].hex(),
                        'packet_class': 'agc / ClientboundUpdateRecipesPacket',
                        'capture_scope': 'Unmodified original server normal resource/tag loading, no-action offline login; network capture, not a reconstructed serializer output.',
                        'payload_scope': 'payload is the exact decompressed packet body excluding packet ID, length prefix and compression envelope; full_packet includes only the packet ID and body.',
                        'payload': base64.b64encode(body).decode(),
                        'payload_sha256': hashlib.sha256(body).hexdigest(),
                        'payload_bytes': len(body), 'declared_recipe_count': count,
                        'full_packet': base64.b64encode(packet).decode(),
                        'full_packet_sha256': hashlib.sha256(packet).hexdigest(),
                        'full_packet_bytes': len(packet),
                        'wire_frame': base64.b64encode(wire_frame).decode(),
                        'wire_frame_sha256': hashlib.sha256(wire_frame).hexdigest(),
                        'wire_frame_bytes': len(wire_frame),
                        'compression_threshold': compression,
                        'username': username, 'offline_uuid': str(uuid.UUID(bytes=bytes(offline_uuid))),
                        'observed_packets': observed, 'sent_packets': sent,
                        'decoded_by_original_stream': False,
                    }
    raise TimeoutError('no original recipe declaration within sixty seconds')


def atomic_json(path, value):
    temp = path.with_name(path.name + '.partial')
    temp.write_text(json.dumps(value, indent=2) + '\n')
    temp.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=25789)
    parser.add_argument('--java', default=JAVA)
    args = parser.parse_args()
    source = ROOT.parent / 'mc121-oracle'
    outer, inner, world = source / 'server-1.21.jar', source / 'server-inner-1.21.jar', source / 'world'
    assert digest(inner) == INNER_SHA256, 'original inner server JAR pin mismatch'
    assert hashlib.sha1(outer.read_bytes()).hexdigest() == OUTER_SHA1, 'original outer server JAR pin mismatch'
    before = snapshot(world)
    directory = Path(tempfile.mkdtemp(prefix='java-recipe-natural-', dir=ROOT / 'runtime'))
    shutil.copytree(world, directory / 'world', copy_function=shutil.copyfile)
    assert before == snapshot(directory / 'world'), 'private full-world copy mismatch'
    shutil.copyfile(source / 'eula.txt', directory / 'eula.txt')
    properties = (source / 'server.properties').read_text()
    updates = {'online-mode': 'false', 'enforce-secure-profile': 'false',
               'server-ip': '127.0.0.1', 'server-port': str(args.port)}
    lines = []
    for line in properties.splitlines():
        key = line.split('=', 1)[0]
        lines.append(key + '=' + updates.pop(key) if key in updates else line)
    lines += [key + '=' + value for key, value in updates.items()]
    (directory / 'server.properties').write_text('\n'.join(lines) + '\n')
    report = {'ok': False, 'kind': 'original Java natural recipe-declaration capture',
              'minecraft': '1.21', 'oracle_inner_sha256': INNER_SHA256,
              'oracle_outer_sha1': OUTER_SHA1, 'oracle_outer_sha256': digest(outer),
              'capture_helper_sha256': digest(Path(__file__)),
              'runtime_directory': str(directory), 'canonical_world_file_count': len(before),
              'canonical_world_snapshot_sha256': snapshot_digest(before),
              'canonical_properties_sha256': digest(source / 'server.properties'),
              'private_properties_sha256': digest(directory / 'server.properties'),
              'world_copy_mode': 'full copy, content-verified before launch',
              'scope': 'Normal no-action offline login; no server recipe/tag mutation, transformed classes, datapack replacement, or gameplay commands.'}
    server = None
    fixture = None
    log_path = directory / 'server.log'
    try:
        with log_path.open('wb') as log:
            server = subprocess.Popen([args.java, '-Xms128M', '-Xmx1024M', '-jar', str(outer),
                                       '--port', str(args.port), 'nogui'], cwd=directory,
                                      stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT,
                                      start_new_session=True)
            deadline = time.monotonic() + 180
            while b'Done (' not in log_path.read_bytes():
                if server.poll() is not None:
                    raise RuntimeError('server exited during startup; see ' + str(log_path))
                if time.monotonic() >= deadline:
                    raise TimeoutError('original server startup deadline')
                time.sleep(0.1)
            launched_inner = directory / 'versions/1.21/server-1.21.jar'
            assert digest(launched_inner) == INNER_SHA256, 'actually launched inner JAR pin mismatch'
            report['launched_inner_sha256'] = digest(launched_inner)
            fixture = capture(args.port)
            report.update(ok=True, payload_sha256=fixture['payload_sha256'],
                          payload_bytes=fixture['payload_bytes'],
                          declared_recipe_count=fixture['declared_recipe_count'])
    except Exception as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        if server is not None and server.poll() is None:
            # Shutdown only; no gameplay or resource-loading command is issued.
            try:
                server.stdin.write(b'stop\n')
                server.stdin.flush()
                server.wait(timeout=20)
            except (OSError, subprocess.TimeoutExpired):
                os.killpg(server.pid, signal.SIGKILL)
                server.wait(timeout=5)
        report['server_exit_code'] = None if server is None else server.returncode
        after = snapshot(world)
        report['reference_world_unchanged'] = before == after
        report['canonical_world_after_snapshot_sha256'] = snapshot_digest(after)
        report['server_log_sha256'] = digest(log_path)
        saved_log = ROOT / 'evidence/server-recipe-book-natural-declarations-server.log'
        shutil.copyfile(log_path, saved_log)
        report['server_log'] = str(saved_log.relative_to(ROOT))
        report['original_jars_unchanged'] = (digest(inner) == INNER_SHA256 and
                                           hashlib.sha1(outer.read_bytes()).hexdigest() == OUTER_SHA1)
        if not report['original_jars_unchanged']:
            report.update(ok=False, error='original server JAR changed')
        if not report['reference_world_unchanged']:
            report.update(ok=False, error='canonical reference world changed')
        if fixture is not None and report['ok']:
            fixture.update(oracle_inner_sha256=INNER_SHA256, oracle_outer_sha1=OUTER_SHA1,
                           oracle_outer_sha256=report['oracle_outer_sha256'],
                           canonical_world_snapshot_sha256=report['canonical_world_snapshot_sha256'],
                           reference_world_unchanged=report['reference_world_unchanged'])
            destination = ROOT / 'fixtures/server-recipe-book-natural-declarations.json'
            atomic_json(destination, fixture)
            report['fixture_sha256'] = digest(destination)
            report['fixture'] = str(destination.relative_to(ROOT))
        atomic_json(ROOT / 'evidence/server-recipe-book-natural-declarations.json', report)
    print(json.dumps(report, indent=2))
    if not report['ok']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
