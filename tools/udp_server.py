#!/usr/bin/env python3
"""
GLOS UDP debug listener.

Receives UDP packets and prints packet statistics
for replayer debugging.
"""

import socket

HOST = "127.0.0.1"
PORT = 5555

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind((HOST, PORT))

print(f"Listening UDP on {HOST}:{PORT}")

packet_count = 0
total_bytes = 0

while True:
    data, addr = sock.recvfrom(65535)

    packet_count += 1
    total_bytes += len(data)

    print(f"packet={packet_count} bytes={len(data)} total={total_bytes}")

    print(data[:32].hex())
