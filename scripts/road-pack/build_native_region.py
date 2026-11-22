#!/usr/bin/env python3
"""Project a reviewed raw OSM XML region into a candidate native road asset.

This is offline build tooling. Its output is not an IPC/import trust source.
"""
import argparse
from collections import defaultdict
from datetime import datetime
import json
import math
import os
from pathlib import Path
import re
import tempfile
import xml.etree.ElementTree as ET

import build

MODES = ('bus', 'tram')
POSITIVE = ('yes', 'designated')
# Deliberately narrow initial street profile. Every other attribute excludes
# the way unless positively classified as immaterial to this profile.
STREETS = {'residential', 'unclassified'}
STRUCTURE = {'bridge': 'no', 'tunnel': 'no', 'layer': '0', 'level': '0', 'area': 'no'}
GENERAL_ACCESS = {'access', 'vehicle', 'motor_vehicle', 'psv'}
WAY_KEYS = {'highway', 'oneway', 'bus', 'tram', 'name', 'surface'} | GENERAL_ACCESS | STRUCTURE.keys()
INPUT_BYTES = 32 * 1024 * 1024
MAX_ITEMS = 200_000
MAX_EDGES = 4096
MAX_TRANSITIONS = 8192
MAX_STOPS = 4096
MAX_OUTPUT_BYTES = 4 * 1024 * 1024
SIMULATED_TURN_ORIGIN = ' [Rosenfu simulated turns]'
POLICY_FIELDS = {
    'format', 'asset_id', 'source_sha256', 'coverage_sha256', 'revision', 'origin', 'region',
    'bounds', 'modes', 'turns', 'bus_stop_positions',
}
JUNCTION_CERTIFICATE = 'simulated_junctions_sha256'
BUS_PROFILE_CERTIFICATE = 'simulated_bus_profile_sha256'
BUS_PROFILE = 'jp-local-simulated-bus-v1'
SIMULATED_BUS_ORIGIN = ' [Rosenfu simulated bus access]'
PROFILE_SURFACES = {'paved', 'asphalt', 'concrete', 'paving_stones'}
PLAYER_BUS_STOPS = 'allows_player_bus_stops'


def unique_json(pairs):
    result = {}
    for key, value in pairs:
        build.require(key not in result, 'duplicate policy or coverage key')
        result[key] = value
    return result


def json_document(path):
    return json.loads(build.read_limited(path, 1024 * 1024), object_pairs_hook=unique_json)


def point_dto(point):
    return {'longitude': point[0], 'latitude': point[1]}


def parse_source(source, source_sha, revision, *, require_snapshot_revision=False):
    _, _, root = build.read_osm_xml(source, INPUT_BYTES, source_sha)
    build.require(root.get('version') == '0.6', 'raw OSM version mismatch')
    build.require(len(root) <= MAX_ITEMS, 'raw OSM element limit')
    metadata = root.findall('meta')
    build.require(len(metadata) <= 1, 'duplicate OSM revision metadata')
    source_revision = metadata[0].get('osm_base') if metadata else None
    if require_snapshot_revision:
        build.require(bool(source_revision), 'simulated bus profile requires raw snapshot revision metadata')
    if source_revision is not None:
        build.require(datetime.fromisoformat(source_revision) == datetime.fromisoformat(revision),
                      'raw OSM revision mismatch')
    nodes, ways, incident = {}, {}, defaultdict(set)
    blocked_nodes, blocked_ways, relations = set(), set(), set()
    for el in root:
        kind = el.tag
        if kind not in ('node', 'way', 'relation'):
            build.require(kind in ('bounds', 'meta', 'note'), 'unsupported raw OSM element')
            continue
        build.require(el.get('action') is None and el.get('visible') in (None, 'true'),
                      'raw OSM region contains a change or hidden entity')
        ident, tags = build.osm_id(el.get('id')), build.read_tags(el)
        if kind == 'node':
            build.require(ident not in nodes, 'duplicate raw OSM node')
            point = build.point((float(el.attrib['lon']), float(el.attrib['lat'])))
            nodes[ident] = point
            if tags:  # including barrier, level and access; no unsupported-node interpolation
                blocked_nodes.add(ident)
        elif kind == 'way':
            build.require(ident not in ways, 'duplicate raw OSM way')
            refs = [build.osm_id(n.get('ref')) for n in el.findall('nd')]
            build.require(len(refs) >= 2, 'raw OSM way missing node references')
            ways[ident] = (refs, tags)
            for ref in refs:
                incident[ref].add(ident)
        else:
            build.require(ident not in relations, 'duplicate raw OSM relation')
            relations.add(ident)
            if (tags.get('type', '').split(':')[0] == 'restriction'
                    or any(k.split(':')[0] == 'restriction' for k in tags)):
                members = el.findall('member')
                build.require(bool(members), 'restriction relation missing members')
                for member in members:
                    member_kind = member.get('type')
                    build.require(member_kind in ('node', 'way'), 'unsupported restriction member')
                    target = blocked_nodes if member_kind == 'node' else blocked_ways
                    target.add(build.osm_id(member.get('ref')))
    build.require(all(ref in nodes for ref in incident), 'raw region has unresolved way nodes')
    return nodes, ways, incident, blocked_nodes, blocked_ways


def mode_access(tags, mode, enabled):
    value = tags.get(mode)
    if value is not None and value not in POSITIVE:
        return 'forbidden' if value in ('no', 'private') else 'unknown'
    return 'allowed' if mode in enabled and value in POSITIVE else 'unknown'


def supported(tags):
    return (tags.keys() <= WAY_KEYS
            and tags.get('highway') in STREETS
            and tags.get('oneway') in ('yes', 'no', '-1')
            and all(tags.get(key, value) == value for key, value in STRUCTURE.items())
            and all(key not in tags or tags[key] in POSITIVE for key in GENERAL_ACCESS))


def supported_simulated_bus(tags):
    source = tags.get('source')
    return (tags.keys() <= WAY_KEYS | {'source'}
            and (source is None or bool(source.strip()) and all(ord(c) >= 32 and ord(c) != 127 for c in source))
            and tags.get('highway') in STREETS
            and tags.get('oneway', 'no') in ('yes', 'no', '-1')
            and tags.get('surface', 'paved') in PROFILE_SURFACES
            and all(tags.get(key, value) == value for key, value in STRUCTURE.items())
            and all(key not in tags or tags[key] in POSITIVE for key in GENERAL_ACCESS))


def verify_simulated_bus_profile(path, policy):
    digest = policy.get(BUS_PROFILE_CERTIFICATE)
    build.require((path is not None) == (BUS_PROFILE_CERTIFICATE in policy),
                  'simulated bus profile requires both policy opt-in and a separate source certificate')
    if path is None:
        return False
    build.require('bus' in policy['modes'], 'simulated bus profile requires enabled bus mode')
    build.require(isinstance(digest, str) and re.fullmatch(r'[0-9a-f]{64}', digest),
                  'invalid simulated bus source certificate digest')
    raw = build.read_limited(path, 1024 * 1024)
    build.require(build.sha(raw) == digest, 'simulated bus source certificate digest mismatch')
    certificate = json.loads(raw, object_pairs_hook=unique_json)
    binding = ('source_sha256', 'coverage_sha256', 'revision', 'bounds')
    attest = ('raw_way_access_and_direction_tags_complete', 'raw_node_tags_complete',
              'incident_ways_complete', 'restriction_relations_complete')
    build.require(isinstance(certificate, dict)
                  and set(certificate) == {'format', 'profile', 'country', *binding, *attest}
                  and type(certificate['format']) is int and certificate['format'] == 1
                  and certificate['profile'] == BUS_PROFILE and certificate['country'] == 'JP'
                  and all(certificate[k] is True for k in attest),
                  'missing affirmative Japanese raw tag, topology or restriction coverage')
    build.require(all(build.encoded(certificate[k]) == build.encoded(policy[k]) for k in binding),
                  'simulated bus certificate does not bind the exact region snapshot')
    west, south, east, north = policy['bounds']
    build.require(122 <= west < east <= 154 and 20 <= south < north <= 46,
                  'simulated bus region outside supported map rectangle')
    return True


def positive_rule(edges, mode, incoming, outgoing):
    build.require(mode in MODES and incoming in edges and outgoing in edges,
                  'reviewed movement references an unavailable edge')
    a, b = edges[incoming], edges[outgoing]
    build.require(a['to'] == b['from'] and a['from'] != b['to']
                  and all(e[f'{mode}_access'] == 'allowed' for e in (a, b)),
                  'reviewed movement is not a supported directed access')


def verify_junction_coverage(path, policy):
    digest = policy.get(JUNCTION_CERTIFICATE)
    build.require((path is not None) == (JUNCTION_CERTIFICATE in policy),
                  'simulated turns require both policy opt-in and a separate junction certificate')
    if path is None:
        return False
    build.require(isinstance(digest, str) and re.fullmatch(r'[0-9a-f]{64}', digest),
                  'invalid junction certificate digest')
    raw = build.read_limited(path, 1024 * 1024)
    build.require(build.sha(raw) == digest, 'junction certificate digest mismatch')
    document = json.loads(raw, object_pairs_hook=unique_json)
    binding = ('source_sha256', 'coverage_sha256', 'revision', 'bounds')
    fields = {'format', 'incident_ways_complete', 'restriction_relations_complete', *binding}
    build.require(isinstance(document, dict) and set(document) == fields
                  and type(document['format']) is int and document['format'] == 1
                  and document['incident_ways_complete'] is True
                  and document['restriction_relations_complete'] is True,
                  'missing affirmative incident-way or turn-restriction coverage')
    build.require(all(build.encoded(document[k]) == build.encoded(policy[k]) for k in binding),
                  'junction certificate does not bind the exact region snapshot')
    return True


def simple_junction_ways(node, accepted_ways, nodes, ways, incident, blocked_nodes):
    raw_ways = incident[node]
    if not (2 <= len(raw_ways) <= 3 and raw_ways <= accepted_ways):
        return None
    arms = []
    for way in raw_ways:
        refs, _ = ways[way]  # accepted ways have unique node refs
        index = refs.index(node)
        arms.extend(refs[max(0, index-1):index])
        arms.extend(refs[index+1:index+2])
    if (len(arms) != 3 or len(set(arms)) != 3 or blocked_nodes.intersection(arms)
            or len({nodes[n] for n in [node, *arms]}) != 4):
        return None
    centre = nodes[node]
    vectors = [(nodes[a][0]-centre[0], nodes[a][1]-centre[1]) for a in arms]
    # Same-side collinear arms can be overlapping ways or a split lane
    # representation, not the bounded simple junction supported here.
    if any(a[0]*b[1] == a[1]*b[0] and a[0]*b[0] + a[1]*b[1] > 0
           for i, a in enumerate(vectors) for b in vectors[i+1:]):
        return None
    return raw_ways


def simple_four_arm_bus_through(node, observed_ways, nodes, ways, incident, blocked_nodes, coverage):
    """Exactly two ground original ways, both nearly straight through their common raw node."""
    ways_here = incident[node]
    if len(ways_here) != 2 or not ways_here <= observed_ways or node in blocked_nodes:
        return False
    arms = []
    for way in sorted(ways_here):
        refs, _ = ways[way]  # profiled ways have unique node refs
        i = refs.index(node)
        if i == 0 or i == len(refs) - 1:
            return False  # do not infer same-road identity across split ways
        arms.extend((refs[i-1], refs[i+1]))
    if (len(set(arms)) != 4 or blocked_nodes.intersection(arms)
            or len({nodes[n] for n in [node, *arms]}) != 5):
        return False
    lon, lat = nodes[node]
    cos_lat = math.cos(math.radians(lat))
    directions = []
    for arm in arms:
        lon1, lat1 = nodes[arm]
        x, y = (lon1-lon)*cos_lat, lat1-lat
        length = math.hypot(x, y)
        directions.append((x/length, y/length))

    def dot(a, b):
        return a[0]*b[0] + a[1]*b[1]

    # At least 30 degrees between arms. Within each raw way, allow no
    # more than a 30-degree turn; centre-line coincidence is not a junction.
    cos30 = math.sqrt(3)/2
    if not (dot(directions[0], directions[1]) <= -cos30
            and dot(directions[2], directions[3]) <= -cos30
            and all(dot(a, b) <= cos30 for a in directions[:2] for b in directions[2:])):
        return False
    # A cross street may extend outside the playable rectangle. Its local
    # arms are evidence only, and must lie strictly inside the certified halo.
    # Keep even an observed arm unavailable if it touches/exits that polygon.
    centre = nodes[node]
    boundary = list(zip(coverage, coverage[1:]))
    return all(build.inside(nodes[arm], coverage)
               and not any(build.intersects(centre, nodes[arm], a, b) for a, b in boundary)
               for arm in arms)


def simulated_junction_rules(edges, accepted_ways, nodes, ways, incident, blocked_nodes):
    """Candidate gameplay rules, never an assertion of legal turns or real access."""
    incoming, outgoing = defaultdict(list), defaultdict(list)
    for edge in edges.values():
        incoming[edge['to'][4:]].append(edge)
        outgoing[edge['from'][4:]].append(edge)
    rules = set()
    for node in incoming.keys() & outgoing.keys():
        raw_ways = simple_junction_ways(node, accepted_ways, nodes, ways, incident, blocked_nodes)
        if raw_ways is None:
            continue
        for mode in MODES:
            # Even a third arm with unknown mode permission defeats automatic
            # calculation for this mode. Road classifications alone prove none.
            if not all(ways[way][1].get(mode) in POSITIVE for way in raw_ways):
                continue
            access = f'{mode}_access'
            for a in incoming[node]:
                for b in outgoing[node]:
                    if a['from'] != b['to'] and a[access] == b[access] == 'allowed':
                        rules.add((mode, a['id'], b['id']))
                        build.require(len(rules) <= MAX_TRANSITIONS, 'native movement limit')
    return rules


def project(source, coverage, policy, *, junction_coverage=None, bus_profile_coverage=None):
    build.require(isinstance(policy, dict)
                  and POLICY_FIELDS <= set(policy) <= POLICY_FIELDS | {JUNCTION_CERTIFICATE, PLAYER_BUS_STOPS, BUS_PROFILE_CERTIFICATE}
                  and type(policy['format']) is int and policy['format'] == 1, 'invalid region policy')
    allows_player_bus_stops = policy.get(PLAYER_BUS_STOPS, False)
    build.require(type(allows_player_bus_stops) is bool, 'player bus stop opt-in must be boolean')
    asset_id, revision = policy['asset_id'], policy['revision']
    build.require(isinstance(asset_id, str) and re.fullmatch(r'[a-z0-9-]{1,64}', asset_id), 'invalid candidate asset ID')
    build.require(isinstance(revision, str) and len(revision) <= 64, 'invalid source revision')
    build.require(datetime.fromisoformat(revision).utcoffset() is not None, 'revision must include time zone')
    build.require(all(isinstance(policy[k], str) and 0 < len(policy[k]) <= 128
                      and policy[k].strip() == policy[k] and all(' ' <= c < '\x7f' for c in policy[k])
                      for k in ('origin', 'region')), 'origin and region must be bounded printable labels')
    modes = policy['modes']
    build.require(isinstance(modes, list) and 0 < len(modes) <= len(MODES)
                  and all(mode in MODES for mode in modes) and len(set(modes)) == len(modes), 'invalid enabled modes')
    for key in ('source_sha256', 'coverage_sha256'):
        build.require(isinstance(policy[key], str) and re.fullmatch(r'[0-9a-f]{64}', policy[key]), 'invalid pinned digest')
    bounds = policy['bounds']
    build.require(isinstance(bounds, list) and len(bounds) == 4, 'invalid regional bounds')
    west, south = build.point(bounds[:2])
    east, north = build.point(bounds[2:])
    build.require(0 < east-west <= .5 and 0 < north-south <= .5, 'regional bounds exceed supported size')
    coverage_bytes = build.read_limited(coverage, 1024 * 1024)
    build.require(build.sha(coverage_bytes) == policy['coverage_sha256'], 'coverage digest mismatch')
    ring = build.polygon(json.loads(coverage_bytes, object_pairs_hook=unique_json))
    build.require(build.covered(bounds, ring), 'regional bounds are not wholly inside certified coverage')
    turns, stops = policy['turns'], policy['bus_stop_positions']
    build.require(isinstance(turns, list) and len(turns) <= MAX_TRANSITIONS
                  and isinstance(stops, list) and len(stops) <= MAX_STOPS, 'region policy limits')
    simulate = verify_junction_coverage(junction_coverage, policy)
    simulated_bus = verify_simulated_bus_profile(bus_profile_coverage, policy)
    origin = (policy['origin'] + (SIMULATED_TURN_ORIGIN if simulate else '')
              + (SIMULATED_BUS_ORIGIN if simulated_bus else ''))
    build.require(len(origin) <= 128, 'origin plus simulation label exceeds limit')
    nodes, ways, incident, blocked_nodes, blocked_ways = parse_source(
        source, policy['source_sha256'], revision, require_snapshot_revision=simulated_bus)
    if simulate or simulated_bus:
        build.require(blocked_nodes <= nodes.keys() and blocked_ways <= ways.keys(),
                      'regional coverage contains an unresolved restriction member')
    edges, used_nodes, sequences, accepted_ways = {}, set(), [], set()
    profiled_bus_ways, observed_bus_ways = set(), set()
    for wid, (refs, tags) in sorted(ways.items()):
        if wid in blocked_ways or len(set(refs)) != len(refs):
            continue
        strict = supported(tags)
        if not simulated_bus and not strict:
            continue
        access = {f'{m}_access': mode_access(tags, m, modes) if strict else 'unknown' for m in MODES}
        if simulated_bus:
            access['bus_access'] = (mode_access({'bus': tags.get('bus', 'yes')}, 'bus', modes)
                                    if supported_simulated_bus(tags) else 'unknown')
        # Tram keeps the original strict evidence and oneway requirements; the
        # gameplay profile never confers a tram edge or a tram direction.
        if 'allowed' not in access.values():
            continue
        if simulated_bus and access['bus_access'] == 'allowed':
            observed_bus_ways.add(wid)
        # Reject partial ways/boundary cuts and preserve the original node order;
        # splitting each surviving way at each raw node preserves true topology.
        if any(not (west < nodes[ref][0] < east and south < nodes[ref][1] < north)
               for ref in refs):
            continue
        accepted_ways.add(wid)
        if simulated_bus and access['bus_access'] == 'allowed':
            profiled_bus_ways.add(wid)
        directed = list(zip(refs, refs[1:]))
        for direction in ('forward', 'reverse'):
            if tags.get('oneway', 'no') == ('-1' if direction == 'forward' else 'yes'):
                continue
            sequence = []
            for n, pair in enumerate(directed):
                a, b = pair if direction == 'forward' else pair[::-1]
                if a in blocked_nodes or b in blocked_nodes or nodes[a] == nodes[b]:
                    sequence.append(None)
                    continue
                edge_id = f'osm:{wid}:{n}:{direction}'
                edges[edge_id] = {
                    'id': edge_id, 'from': f'osm:{a}', 'to': f'osm:{b}',
                    'source': f'native-pilot:{asset_id}', 'revision': revision,
                    'geometry': [point_dto(nodes[a]), point_dto(nodes[b])], 'level': 'ground', **access,
                }
                used_nodes.update((a, b))
                sequence.append(edge_id)
                build.require(len(edges) <= MAX_EDGES, 'native directed edge limit')
            sequences.append(sequence if direction == 'forward' else sequence[::-1])
    build.require(bool(edges), 'region has no supported positive-access edges')
    rules = set()
    through_nodes = {}
    for sequence in sequences:
        for a, b in zip(sequence, sequence[1:]):
            if a is None or b is None:
                continue
            node = edges[a]['to'][4:]
            shared = len(incident[node]) != 1
            if shared:
                if not simulated_bus:
                    continue
                if node not in through_nodes:
                    through_nodes[node] = (
                        simple_junction_ways(node, profiled_bus_ways, nodes, ways, incident, blocked_nodes) is not None
                        or simple_four_arm_bus_through(
                            node, observed_bus_ways, nodes, ways, incident, blocked_nodes, ring)
                    )
                if not through_nodes[node]:
                    continue
            for mode in MODES:
                if (not shared or mode == 'bus') and edges[a][f'{mode}_access'] == 'allowed':
                    rules.add((mode, a, b))
    for turn in turns:
        build.require(isinstance(turn, dict) and set(turn) == {'mode', 'incoming', 'outgoing'}
                      and all(isinstance(v, str) for v in turn.values()), 'invalid reviewed movement')
        mode, a, b = turn['mode'], turn['incoming'], turn['outgoing']
        positive_rule(edges, mode, a, b)
        build.require((mode, a, b) not in rules, 'duplicate movement')
        rules.add((mode, a, b))
    if simulate:
        rules |= simulated_junction_rules(edges, accepted_ways, nodes, ways, incident, blocked_nodes)
    build.require(len(rules) <= MAX_TRANSITIONS, 'native movement limit')
    reviewed_stops = set()
    for stop in stops:
        build.require(isinstance(stop, dict) and set(stop) == {'edge_id', 'position'}
                      and isinstance(stop['edge_id'], str), 'invalid reviewed boarding location')
        key, fraction = stop['edge_id'], stop['position']
        build.require(key in edges and edges[key]['bus_access'] == 'allowed'
                      and type(fraction) in (int, float) and 0 <= fraction <= 1,
                      'boarding location is not on a supported bus edge')
        build.require((key, fraction) not in reviewed_stops, 'duplicate boarding location')
        reviewed_stops.add((key, fraction))
    asset = {
        'format': 1, 'asset_id': asset_id, 'revision': revision, 'origin': origin,
        'region': policy['region'], 'attribution': '© OpenStreetMap contributors (ODbL-1.0)',
        'nodes': sorted(({'id': f'osm:{n}', 'point': point_dto(nodes[n])} for n in used_nodes), key=lambda n: n['id']),
        'edges': [edges[key] for key in sorted(edges)],
        'transitions': [{'mode': mode, 'incoming': a, 'outgoing': b} for mode, a, b in sorted(rules)],
        'bus_stop_positions': [{'edge_id': key, 'position': fraction} for key, fraction in sorted(reviewed_stops)],
    }
    if allows_player_bus_stops:
        asset[PLAYER_BUS_STOPS] = True
    return asset


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('source', type=Path)
    p.add_argument('--coverage', required=True, type=Path)
    p.add_argument('--policy', required=True, type=Path)
    p.add_argument('--junction-coverage', type=Path, help='separate simulated-junction coverage certificate')
    p.add_argument('--bus-profile-coverage', type=Path, help='separate raw Japanese road coverage certificate')
    p.add_argument('--output', required=True, type=Path)
    args = p.parse_args()
    try:
        policy_bytes = build.read_limited(args.policy, 1024 * 1024)
        policy = json.loads(policy_bytes, object_pairs_hook=unique_json)
        asset = project(args.source, args.coverage, policy, junction_coverage=args.junction_coverage,
                        bus_profile_coverage=args.bus_profile_coverage)
        payload = build.encoded(asset)
        build.require(len(payload) <= MAX_OUTPUT_BYTES, 'native asset byte limit')
        # A successful output never changes an existing artifact. Root validates
        # the complete payload and records its digest before any trusted pin.
        with tempfile.TemporaryDirectory(prefix='.native-region-', dir=args.output.parent) as temp:
            staged = Path(temp) / 'asset.json'
            staged.write_bytes(payload)
            os.link(staged, args.output)  # publish complete bytes without replacing an existing file
        receipt = {'sha256': build.sha(payload), 'source_sha256': policy['source_sha256'],
                   'coverage_sha256': policy['coverage_sha256'], 'policy_sha256': build.sha(policy_bytes),
                   'revision': asset['revision'], 'edges': len(asset['edges']),
                   'movements': len(asset['transitions']), 'candidate_only': True}
        if args.junction_coverage is not None:
            receipt.update(movement_policy='simulated-simple-at-grade-v1',
                           simulated_junctions_sha256=policy[JUNCTION_CERTIFICATE])
        if args.bus_profile_coverage is not None:
            receipt.update(bus_profile=BUS_PROFILE, simulated_bus_profile_sha256=policy[BUS_PROFILE_CERTIFICATE])
        if asset.get(PLAYER_BUS_STOPS):
            receipt[PLAYER_BUS_STOPS] = True
        print(json.dumps(receipt))
    except (OSError, ValueError, KeyError, ET.ParseError) as error:
        p.exit(2, f'Native candidate rejected: {error}\n')


if __name__ == '__main__':
    main()
