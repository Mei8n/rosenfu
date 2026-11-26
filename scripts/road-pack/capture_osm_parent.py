#!/usr/bin/env python3
"""Capture a bounded native-road staging XML from a local SHA-pinned OSM PBF.

PyOsmium object-stream scans only; no all-parent coordinate index.
Never emits a gameplay certificate or a claim of parent/polygon completeness.
"""
import argparse
from collections import defaultdict
from datetime import datetime
from hashlib import sha256
from importlib.metadata import version
import json
import os
import stat
import tempfile
from pathlib import Path
import re
import xml.etree.ElementTree as ET

import build

try:
    import osmium
except ImportError:
    osmium = None  # optional; the XML/native tools do not require it

need = build.require


def region(value):
    try:
        west, south, east, north = map(float, value.split(','))
        build.point((west, south))
        build.point((east, north))
        need(0 < east-west <= .5 and 0 < north-south <= .5, 'regional bounds exceed supported size')
        return west, south, east, north
    except ValueError as error:
        raise argparse.ArgumentTypeError(str(error)) from error


def contains(point, box):
    return box[0] <= point[0] <= box[2] and box[1] <= point[1] <= box[3]

MAX_SEEDS = 60_000
MAX_WAYS = 20_000
MAX_NODE_REFS = 150_000
MAX_RELATIONS = 4_000
MAX_NESTED_ROUNDS = 4
MAX_RAW_BYTES = 4 * 1024**3
MAX_STAGE_BYTES = 32 * 1024**2
MAX_RETAINED_ESTIMATE = 12 * 1024**2
MAX_OBJECT_REFS = 8192
KINDS = {'n':'node','w':'way','r':'relation'}


def input_identity(path):
    info = path.lstat()
    need(stat.S_ISREG(info.st_mode), 'parent must be a regular local file')
    need(0 < info.st_size <= MAX_RAW_BYTES, 'parent file byte limit')
    return info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns


def digest(path):
    before = input_identity(path)
    h = sha256()
    seen = 0
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024*1024), b''):
            seen += len(chunk)
            need(seen <= before[2], 'parent grew during digest')
            h.update(chunk)
    need(seen == before[2] and input_identity(path) == before, 'parent changed during digest')
    return h.hexdigest()


def scan(path, kind, callback):
    class Capture(osmium.SimpleHandler):
        pass
    setattr(Capture,kind,lambda self, obj: callback(obj))
    bit = {'node':osmium.osm.NODE,'way':osmium.osm.WAY,'relation':osmium.osm.RELATION}[kind]
    with osmium.io.Reader(str(path),bit) as reader:
        osmium.apply(reader,Capture())


def raw_record(obj,payload):
    need(obj.visible, 'selected object is not current visible OSM data')
    need(len(obj.tags) <= 256, 'selected object tag count limit')
    tags=tuple(sorted((tag.k,tag.v) for tag in obj.tags))
    need(len({k for k, _ in tags}) == len(tags), 'duplicate selected object tag')
    need(all(0 < len(k) <= 256 and len(v) <= 4096 for k,v in tags), 'selected object tag length limit')
    return obj.version,tags,payload


def insert(collection,k,value,limit):
    need(0 < k < 2**63, 'selected entity must have a positive OSM ID')
    need(k not in collection, 'duplicate selected object; snapshot state invalid')
    need(len(collection)<limit,'selected object limit; shrink source halo')
    collection[k]=value


def relation_members(obj):
    need(len(obj.members)<=MAX_OBJECT_REFS,'relation member limit')
    members=tuple((KINDS.get(v.type),v.ref,v.role) for v in obj.members)
    need(all(kind is not None and 0 < ref < 2**63 and len(role)<=256 for kind,ref,role in members),
         'unsupported relation member kind or role')
    return members


def snapshot_header(path):
    with osmium.io.Reader(str(path),osmium.osm.NODE) as reader:
        header=reader.header()
        need(not header.has_multiple_object_versions,'history parent is not a current-state snapshot')
        revision=header.get('osmosis_replication_timestamp')
        need(revision and datetime.fromisoformat(revision).utcoffset() is not None,
             'parent lacks an immutable timezone-bearing replication timestamp')
        return revision


def capture(path, parent_sha, halo, inner):
    need(osmium is not None and version('osmium') == '4.3.2', 'use validated pyosmium 4.3.2')
    for box in (halo, inner):
        west, south = build.point(box[:2])
        east, north = build.point(box[2:])
        need(0 < east-west <= .5 and 0 < north-south <= .5, 'regional bounds exceed supported size')
    need(halo[0]<inner[0] and halo[1]<inner[1] and inner[2]<halo[2] and inner[3]<halo[3],
         'strict inner envelope needs a halo')
    need(isinstance(parent_sha, str) and re.fullmatch(r'[0-9a-f]{64}',parent_sha), 'invalid pinned parent SHA256')
    identity = input_identity(path)
    need(digest(path)==parent_sha,'parent release digest mismatch')
    revision=snapshot_header(path)

    def scan_parent(kind, callback):
        need(input_identity(path) == identity, 'parent changed during capture')
        scan(path, kind, callback)
        need(input_identity(path) == identity, 'parent changed during capture')
    nodes,ways,relations={},{},{}
    refs_seen=set()
    retained_estimate=0

    def keep_record(target, obj, payload, limit, kind):
        nonlocal retained_estimate
        value=raw_record(obj,payload)
        tag_size=sum(32+len(k.encode())+len(v.encode()) for k,v in value[1])
        payload_size = (sum(96+len(role.encode()) for _,_,role in payload)
                        if kind=='relation' else len(payload)*32)
        retained_estimate += 120+tag_size+payload_size
        need(retained_estimate <= MAX_RETAINED_ESTIMATE,'retained data budget; shrink source halo')
        insert(target,obj.id,value,limit)

    def seed(obj):
        if obj.location.valid():
            p=(obj.location.lon,obj.location.lat)
            if contains(p,halo):
                keep_record(nodes,obj,p,MAX_SEEDS,'node')
    scan_parent('node',seed)
    seeds=set(nodes)
    need(seeds,'no raw nodes in capture halo')

    def keep_way(obj):
        need(2<=len(obj.nodes)<=MAX_OBJECT_REFS,'selected way reference limit')
        refs=tuple(ref.ref for ref in obj.nodes)
        keep_record(ways,obj,refs,MAX_WAYS,'way')
        refs_seen.update(refs)
        need(len(refs_seen)<=MAX_NODE_REFS,'selected way reference limit')

    def incident(obj):
        if any(ref.ref in seeds for ref in obj.nodes):
            keep_way(obj)
    scan_parent('way',incident)
    need(ways,'no raw ways incident on halonodes')
    initial_way_count=len(ways)
    impacted_nodes=seeds | refs_seen
    impacted_ways=set(ways)

    def touches(members):
        return any((kind=='node' and ref in impacted_nodes) or
                   (kind=='way' and ref in impacted_ways) for kind,ref,_ in members)

    initial_restrictions={}
    def collect_restrictions(obj):
        if not build.is_turn_restriction({tag.k:tag.v for tag in obj.tags}):
            return
        members=relation_members(obj)
        # Keep only restrictions with definite overlap, or a relation wrapper
        # whose descendants must be inspected to exclude overlap.
        if touches(members) or any(kind=='relation' for kind,_,_ in members):
            keep_record(initial_restrictions,obj,members,MAX_RELATIONS,'relation')
    scan_parent('relation',collect_restrictions)
    relations.update(initial_restrictions)
    unresolved=set()
    for _ in range(MAX_NESTED_ROUNDS):
        wanted={ref for _,_,members in relations.values() for kind,ref,_ in members
                if kind=='relation' and ref not in relations and ref not in unresolved}
        if not wanted:
            break
        def child(obj):
            if obj.id in wanted:
                keep_record(relations,obj,relation_members(obj),MAX_RELATIONS,'relation')
        scan_parent('relation',child)
        unresolved.update(wanted-relations.keys())

    # Propagate observed overlap up relation member chains (cycles supported).
    direct={k for k,(_,_,members) in relations.items() if touches(members)}
    parents=defaultdict(set)
    for k,(_,_,members) in relations.items():
        for kind,ref,_ in members:
            if kind=='relation':
                parents[ref].add(k)
    reachable=set(direct)
    wave=set(direct)
    while wave:
        wave={p for k in wave for p in parents[k]}-reachable
        reachable.update(wave)
    selected=reachable & initial_restrictions.keys()
    keep=set(selected)
    frontier=set(selected)
    unresolved_selected=set()
    while frontier:
        next_frontier={ref for k in frontier for kind,ref,_ in relations[k][2]
                       if kind=='relation' and ref not in keep}
        unresolved_selected.update(next_frontier-relations.keys())
        frontier=next_frontier & relations.keys()
        keep.update(frontier)
    kept_relations={k:relations[k] for k in keep}
    # Unknown relation scope (including depth cap) is reported, never silently
    # equivalent to absence. It does not turn unrelated objects into topology.
    uncertain_ids={ref for _,_,members in relations.values() for kind,ref,_ in members
                   if kind=='relation' and ref not in relations}

    needed_ways={ref for _,_,members in kept_relations.values() for kind,ref,_ in members
                 if kind=='way' and ref not in ways}
    if needed_ways:
        def extra_way(obj):
            if obj.id in needed_ways:
                keep_way(obj)
        scan_parent('way',extra_way)
    needed_nodes=refs_seen | {ref for _,_,members in kept_relations.values() for kind,ref,_ in members
                             if kind=='node'}
    need(len(needed_nodes)<=MAX_NODE_REFS,'selected node reference limit')
    needed_nodes -= nodes.keys()
    if needed_nodes:
        def extra_node(obj):
            if obj.id in needed_nodes and obj.location.valid():
                keep_record(nodes,obj,(obj.location.lon,obj.location.lat),MAX_NODE_REFS,'node')
        scan_parent('node',extra_node)
    missing={
        'node':sorted(needed_nodes-nodes.keys()),
        'way':sorted(needed_ways-ways.keys()),
        'relation':sorted(unresolved_selected),
    }
    data={'node':nodes,'way':ways,'relation':kept_relations}
    receipt={
        'format':1, 'parent_sha256':parent_sha, 'revision':revision,
        'halo':list(halo),'inner':list(inner), 'halo_seed_nodes':len(seeds),
        'retained_selection_estimate_bytes':retained_estimate,
        'halo_incident_ways':initial_way_count, 'observed_restriction_ids':sorted(selected),
        'unknown_relation_member_scope_ids':sorted(uncertain_ids),
        'missing_selected_references':missing, 'stage_counts':{kind:len(rows) for kind,rows in data.items()},
        'source_database_completeness_attested':False,
        'can_emit_gameplay_certificate':False,
        'scope':('Within supplied parent only: all observed ways incident on raw halo nodes, full ordered '
                 'way refs and relevant restriction members. Does not prove polygon-wide completeness or '
                 'recover crossing ways with no raw halo node. No actual use or direction asserted.'),
    }
    # Same immutable parent path for every pass; detect a race/replacement before publication.
    need(input_identity(path) == identity and digest(path)==parent_sha,'parent file changed during capture')
    return data, receipt


def encode(data,receipt):
    root=ET.Element('osm',version='0.6',generator='Rosenfu source-side staging only')
    west,south,east,north=receipt['halo']
    ET.SubElement(root,'bounds',minlon=str(west),minlat=str(south),maxlon=str(east),maxlat=str(north))
    ET.SubElement(root,'meta',osm_base=receipt['revision'])
    for kind in ('node','way','relation'):
        for ref,(version,tags,payload) in sorted(data[kind].items()):
            obj=ET.SubElement(root,kind,id=str(ref),version=str(version))
            if kind=='node':
                obj.attrib.update(lon=str(payload[0]),lat=str(payload[1]))
            elif kind=='way':
                for node in payload:
                    ET.SubElement(obj,'nd',ref=str(node))
            else:
                for member_kind,member_id,role in payload:
                    ET.SubElement(obj,'member',type=member_kind,ref=str(member_id),role=role)
            for k,v in tags:
                ET.SubElement(obj,'tag',k=k,v=v)
    raw=ET.tostring(root,encoding='utf-8',xml_declaration=True)+b'\n'
    need(len(raw)<=MAX_STAGE_BYTES,'staging XML byte limit; reduce halo')
    return raw


def publish_stage(output, raw, receipt):
    # The receipt is the commit marker. Copy nothing into an existing name;
    # incomplete directories are invalid stage input and are never certified.
    document = dict(receipt, stage_sha256=build.sha(raw))
    with tempfile.TemporaryDirectory(prefix='.source-stage-', dir=output.parent) as scratch:
        private = Path(scratch)
        (private/'halo.staging.osm').write_bytes(raw)
        (private/'receipt.json').write_bytes((json.dumps(document,ensure_ascii=False,sort_keys=True)+'\n').encode())
        output.mkdir(mode=0o700, exist_ok=False)
        created = []
        try:
            for name in ('halo.staging.osm', 'receipt.json'):
                dest = output/name
                os.link(private/name, dest)
                created.append(dest)
        except OSError:
            for dest in reversed(created):
                dest.unlink()
            output.rmdir()
            raise
    return document


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('parent_pbf',type=Path)
    parser.add_argument('--sha256',required=True)
    parser.add_argument('--halo',required=True,type=region)
    parser.add_argument('--inner',required=True,type=region)
    parser.add_argument('--out-dir',required=True,type=Path)
    args=parser.parse_args()
    try:
        need(re.fullmatch(r'[0-9a-f]{64}',args.sha256),'invalid pinned parent SHA256')
        need(not args.out_dir.exists(),'output directory already exists')
        data,receipt=capture(args.parent_pbf,args.sha256,args.halo,args.inner)
        raw=encode(data,receipt)
        published=publish_stage(args.out_dir,raw,receipt)
        print(json.dumps(published,ensure_ascii=False,sort_keys=True))
    except (ValueError,OSError,RuntimeError) as exc:
        parser.exit(2,f'Parent stage rejected: {exc}\n')


if __name__=='__main__':
    main()
