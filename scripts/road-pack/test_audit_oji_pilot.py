import unittest
import xml.etree.ElementTree as ET

import audit_oji_pilot as pilot


def sample():
    root = ET.Element("osm")
    for mode, wid, start, end, key, value in pilot.SLICES:
        way = ET.SubElement(root, "way", id=wid)
        ET.SubElement(way, "tag", k=key, v=value)
        ET.SubElement(way, "tag", k="oneway", v="yes")
        for i, node in enumerate((start, end)):
            ET.SubElement(way, "nd", ref=node)
            ET.SubElement(root, "node", id=node, lat=str(35.7501 + i / 10000), lon="139.737")
    return root


class PilotAuditTests(unittest.TestCase):
    def test_positive_snapshot_observation_never_grants_routing(self):
        result = pilot.audit(sample())
        self.assertEqual(result["result"], "snapshot_topology_only")
        self.assertFalse(result["routing_authorized"])
        self.assertEqual(set(result["slices"]), {"road", "tram"})

    def test_pinned_byte_gate(self):
        for raw in (b"<osm/>", b"x" * (pilot.LIMIT + 1)):
            with self.assertRaisesRegex(ValueError, "pinned"):
                pilot.check(raw)

    def test_missing_node_is_not_inferred(self):
        root = sample()
        root.remove(root.find("node"))
        with self.assertRaises(KeyError):
            pilot.audit(root)

    def test_boundary_is_not_coverage(self):
        root = sample()
        root.find("node").set("lat", str(pilot.BOUNDS[1]))
        with self.assertRaisesRegex(ValueError, "boundary"):
            pilot.audit(root)

    def test_coincidence_is_not_shared_node_but_exact_id_is(self):
        root = sample()
        ET.SubElement(ET.SubElement(root, "way", id="outside"), "nd", ref="different-id")
        self.assertFalse(pilot.audit(root)["routing_authorized"])
        ET.SubElement(root.findall("way")[-1], "nd", ref=pilot.SLICES[0][2])
        with self.assertRaisesRegex(ValueError, "shared"):
            pilot.audit(root)

    def test_unknown_and_conditional_access(self):
        root = sample()
        ET.SubElement(root.find("way"), "tag", k="psv:conditional", v="no @ (Mo-Fr)")
        with self.assertRaisesRegex(ValueError, "conditional"):
            pilot.audit(root)
        root = sample()
        root.find('way/tag[@k="oneway"]').set("v", "reversible")
        with self.assertRaisesRegex(ValueError, "direction"):
            pilot.audit(root)

    def test_via_way_and_modal_restriction_are_rejected(self):
        for member_type, member_ref in (("way", pilot.SLICES[0][1]), ("node", pilot.SLICES[0][2])):
            root = sample()
            restriction = ET.SubElement(root, "relation", id="abc")
            ET.SubElement(restriction, "tag", k="restriction:bus", v="no_u_turn")
            ET.SubElement(restriction, "member", role="via", type=member_type, ref=member_ref)
            with self.assertRaisesRegex(ValueError, "restriction"):
                pilot.audit(root)


if __name__ == "__main__":
    unittest.main()
