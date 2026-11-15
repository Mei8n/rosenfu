"""Validate the offline leaf contract before publishing a composite."""
import re
from urllib.parse import urlsplit
from build import Collector,MAX_FEATURES,MAX_COORDS,check_bounds,geometry_points


def text(value,limit=4096):
    if not isinstance(value,str) or not 0<len(value)<=limit:raise ValueError('Invalid evidence text')


def validate_leaf(m,d):
    if not isinstance(m,dict) or not isinstance(d,dict):raise ValueError('Expected evidence objects')
    for document in (m,d):
        if type(document.get('version')) is not int or document['version']!=1:raise ValueError('Unsupported evidence version')
        if document.get('geographic_completeness')!='unknown' or document.get('coverage')!=[]:raise ValueError('Evidence cannot certify coverage')
        check_bounds(document.get('bounds'))
        text(document.get('revision'))
    source=m.get('source');entry=m.get('payload')
    if not isinstance(source,dict) or not isinstance(entry,dict):raise ValueError('Missing evidence provenance')
    for field in ('url','license','license_url','attribution','selection'):text(source.get(field))
    for field in ('url','license_url'):
        url=source[field];parsed=urlsplit(url)
        if parsed.scheme!='https' or not parsed.hostname or parsed.username or parsed.password or any(c.isspace() for c in url):raise ValueError('Invalid evidence URL')
        parsed.port  # Reject invalid or out-of-range port syntax.
        host=parsed.hostname.encode('idna').decode('ascii')
        if not re.fullmatch(r'[A-Za-z0-9.:-]+',host):raise ValueError('Invalid evidence URL host')
    if not isinstance(source.get('file_sha256'),str) or not re.fullmatch('[a-f0-9]{64}',source['file_sha256']):raise ValueError('Invalid source hash')
    if type(source.get('bytes')) is not int or not 1<=source['bytes']<=9007199254740991:raise ValueError('Invalid source byte count')
    limitations=m.get('limitations')
    if not isinstance(limitations,list) or not 1<=len(limitations)<=32:raise ValueError('Missing limitations')
    for value in limitations:text(value)
    observations=d.get('observations');issues=d.get('source_issues')
    if not isinstance(observations,list) or len(observations)>MAX_FEATURES or not isinstance(issues,list) or len(issues)>1000:raise ValueError('Missing observations/issues or limit')
    for issue in issues:
        if not isinstance(issue,dict):raise ValueError('Invalid source issue')
        text(issue.get('id'),256);text(issue.get('reason'))
    actual={'water-area':0,'waterway-line':0,'coastline':0};seen=set();coordinates=polygons=0
    for observation in observations:
        if not isinstance(observation,dict) or not isinstance(observation.get('geometry'),dict):raise ValueError('Invalid observation')
        points=geometry_points(observation['geometry']);x,y=points[0]
        collector=Collector([max(-180,x-.001),max(-85,y-.001),min(180,x+.001),min(85,y+.001)])
        collector.add(observation.get('id'),observation.get('kind'),observation['geometry'],observation.get('tags'))
        identity=(observation['kind'],observation['id'])
        if identity in seen:raise ValueError('Duplicate observation')
        seen.add(identity);actual[observation['kind']]+=1;coordinates+=len(points);polygons+=collector.polygons
        if coordinates>MAX_COORDS or polygons>MAX_FEATURES:raise ValueError('Evidence aggregate limit')
    counts=entry.get('counts')
    if not isinstance(counts,dict) or any(type(counts.get(k)) is not int or counts[k]!=v for k,v in actual.items()):raise ValueError('Evidence counts mismatch')
