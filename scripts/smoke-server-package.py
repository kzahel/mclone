#!/usr/bin/env python3
"""Launch a packaged server, receive its first protocol frame, and retain a world."""
from pathlib import Path
from contextlib import closing
import re
import socket
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time

binary = str(Path(sys.argv[1]).resolve())


def receive_exact(client, size):
    data = b''
    while len(data) < size:
        part = client.recv(size - len(data))
        if not part:
            raise RuntimeError('Server closed during its handshake')
        data += part
    return data


with tempfile.TemporaryDirectory(prefix='mclone server smoke ') as temporary:
    root = Path(temporary)
    log = root / 'server.log'
    with log.open('w') as output:
        process = subprocess.Popen([binary, '--listen', '127.0.0.1:0', '--disable-udp',
            '--serve-once', '--world-dir', str(root / 'world')], cwd=root,
            stdout=output, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 90
            while True:
                match = re.search(r'listening on 127\.0\.0\.1:(\d+)', log.read_text())
                if match:
                    break
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Packaged server did not start: ' + log.read_text())
                time.sleep(0.1)
            with socket.create_connection(('127.0.0.1', int(match[1])), timeout=60) as client:
                # Minimal reliable-only handshake from mclone-net's native wire
                # contract. Use the packaged server's advertised version.
                version = int(re.search(r'protocol (\d+)', log.read_text())[1])
                magic = b'MCLONE_NATIVE_TCP'
                name = b'Package smoke'
                hello = magic + struct.pack('<IQ', version, 0) + bytes([1]) * 16 + bytes([len(name)]) + name
                client.sendall(struct.pack('<I', len(hello)) + hello)
                size = struct.unpack('<I', receive_exact(client, 4))[0]
                if size > 1024:
                    raise RuntimeError('Unbounded server handshake')
                reply = receive_exact(client, size)
                if reply != magic + bytes([1]) + struct.pack('<IIQ', version, version, 0):
                    raise RuntimeError('Server did not accept the current reliable protocol handshake')
                # ClientCommand::Disconnect(ClientDisconnectReason::Quit).
                client.sendall(struct.pack('<I', 2) + bytes([11, 0]))
                while client.recv(4096):
                    if time.monotonic() > deadline:
                        raise RuntimeError('Server did not acknowledge client quit')
            if process.wait(timeout=60) != 0:
                raise RuntimeError('Server did not close cleanly: ' + log.read_text())
            with closing(sqlite3.connect(root / 'world/world.sqlite3')) as database:
                assert database.execute('pragma integrity_check').fetchone() == ('ok',)
                assert database.execute("select count(*) from sqlite_master where type='table'").fetchone()[0] > 0
            print('Packaged server startup, first protocol data, shutdown and SQLite integrity passed.')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
