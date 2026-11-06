# SPDX-License-Identifier: GPL-2.0-only
import copy
import unittest
from diff import compare

class OrderedTraceTests(unittest.TestCase):
    def setUp(self):
        self.trace = {'ok': True, 'phases': [{'name': 'place', 'events': [
            {'name': 'block_change', 'state': 1},
            {'name': 'acknowledge_player_digging', 'sequence': 101}]}]}

    def test_identical(self):
        self.assertEqual(compare(self.trace, copy.deepcopy(self.trace)), [])

    def test_order_is_not_hidden(self):
        other = copy.deepcopy(self.trace)
        other['phases'][0]['events'].reverse()
        self.assertEqual(compare(self.trace, other)[0]['kind'], 'ordered-trace')

    def test_duplicate_and_missing_phase_are_not_hidden(self):
        other = copy.deepcopy(self.trace)
        other['phases'][0]['events'].append({'name': 'block_change', 'state': 1})
        self.assertTrue(compare(self.trace, other))
        self.assertTrue(compare(self.trace, {'ok': True, 'phases': []}))

    def test_two_failed_runs_do_not_pass(self):
        failed = {'ok': False, 'error': 'timeout', 'phases': []}
        self.assertEqual(len(compare(failed, failed)), 2)

if __name__ == '__main__':
    unittest.main()
