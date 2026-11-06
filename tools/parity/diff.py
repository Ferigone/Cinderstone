#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Exact ordered comparison of the scenario's explicitly selected observations."""

def compare(java, native):
    differences = []
    for label, trace in [('java', java), ('native', native)]:
        if not trace.get('ok'):
            differences.append({'kind': 'execution', 'server': label, 'error': trace.get('error')})
    left, right = java.get('phases', []), native.get('phases', [])
    for i in range(max(len(left), len(right))):
        a = left[i] if i < len(left) else None
        b = right[i] if i < len(right) else None
        if a != b:
            differences.append({'kind': 'ordered-trace', 'phase_index': i, 'java': a, 'native': b})
    return differences
